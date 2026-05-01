//! CryptDoor privileged helper.
//!
//! On macOS: runs as root via launchd, listens on a Unix domain socket.
//! On Windows: runs as a Windows Service (LocalSystem), listens on a Named Pipe.
//!
//! The protocol is the same on both: line-delimited JSON commands.

use interprocess::local_socket::traits::{ListenerExt as _, Stream as _};
use interprocess::local_socket::{prelude::*, ListenerOptions, Stream};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

const HELPER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "macos")]
const SOCKET_PATH: &str = "/var/run/online.cryptdoor.helper.sock";

#[cfg(target_os = "macos")]
const LOG_PATH: &str = "/var/log/cryptdoor-helper.log";

#[cfg(target_os = "windows")]
const PIPE_NAME: &str = "online.cryptdoor.helper";

#[cfg(target_os = "windows")]
const SERVICE_NAME: &str = "CryptDoorHelper";

#[cfg(target_os = "windows")]
const SERVICE_DISPLAY_NAME: &str = "CryptDoor Helper";

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
enum Request {
    Version,
    Status,
    Start {
        mihomo: String,
        config: String,
        workdir: String,
    },
    Stop,
}

#[derive(Debug, Serialize)]
struct Response {
    ok: bool,
    error: Option<String>,
    data: Option<serde_json::Value>,
}

impl Response {
    fn ok(data: serde_json::Value) -> Self {
        Self { ok: true, error: None, data: Some(data) }
    }
    fn ok_empty() -> Self {
        Self { ok: true, error: None, data: None }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self { ok: false, error: Some(msg.into()), data: None }
    }
}

static CHILD: Mutex<Option<Child>> = Mutex::new(None);

fn log(msg: &str) {
    let line = format!("[{}] {}\n", chrono_now(), msg);
    eprint!("{line}");
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_PATH)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(log_path) = log_path_windows() {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
            {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn log_path_windows() -> std::io::Result<PathBuf> {
    let dir = std::env::var("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\ProgramData"))
        .join("CryptDoor");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("helper.log"))
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn handle(req: Request) -> Response {
    match req {
        Request::Version => Response::ok(serde_json::json!({ "version": HELPER_VERSION })),
        Request::Status => {
            let mut guard = CHILD.lock().expect("lock");
            let running = match guard.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(None) => true,
                    _ => {
                        *guard = None;
                        false
                    }
                },
                None => false,
            };
            Response::ok(serde_json::json!({ "running": running }))
        }
        Request::Start { mihomo, config, workdir } => {
            stop_inner();
            kill_stray_mihomo();

            let workdir = PathBuf::from(workdir);
            if let Err(e) = fs::create_dir_all(&workdir) {
                return Response::err(format!("create workdir failed: {e}"));
            }
            let cfg_path = workdir.join("config.yaml");
            if let Err(e) = fs::write(&cfg_path, &config) {
                return Response::err(format!("write config failed: {e}"));
            }

            let log_file = match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(workdir.join("mihomo.log"))
            {
                Ok(f) => f,
                Err(e) => return Response::err(format!("open log failed: {e}")),
            };
            let log_err = match log_file.try_clone() {
                Ok(f) => f,
                Err(e) => return Response::err(format!("clone log failed: {e}")),
            };

            log(&format!("starting mihomo: {} -d {:?}", mihomo, workdir));
            let child = match Command::new(&mihomo)
                .arg("-d")
                .arg(&workdir)
                .arg("-f")
                .arg(&cfg_path)
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_err))
                .stdin(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => return Response::err(format!("spawn mihomo failed: {e}")),
            };

            log(&format!("mihomo pid={}", child.id()));
            *CHILD.lock().expect("lock") = Some(child);

            Response::ok(serde_json::json!({ "pid": null }))
        }
        Request::Stop => {
            stop_inner();
            Response::ok_empty()
        }
    }
}

#[cfg(target_os = "macos")]
fn kill_stray_mihomo() {
    let patterns = [
        "cryptdoor-app/src-tauri/target/.*/mihomo",
        "cursor-sandbox-cache/.*/mihomo",
        "cryptdoor-app/.*sidecar/mihomo",
    ];
    for p in patterns {
        let _ = Command::new("/usr/bin/pkill")
            .args(["-9", "-f", p])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
}

#[cfg(target_os = "windows")]
fn kill_stray_mihomo() {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "mihomo.exe"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(200));
}

fn stop_inner() {
    let mut guard = CHILD.lock().expect("lock");
    if let Some(mut child) = guard.take() {
        log(&format!("killing mihomo pid={}", child.id()));
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn handle_client(stream: Stream) {
    let (recv, mut send) = stream.split();
    let reader = BufReader::new(recv);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => return,
        };
        if line.trim().is_empty() {
            continue;
        }

        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(req),
            Err(e) => Response::err(format!("invalid request: {e}")),
        };

        let mut payload = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
        payload.push(b'\n');
        if send.write_all(&payload).is_err() {
            return;
        }
        let _ = send.flush();
    }
}

fn run_server() {
    log(&format!("cryptdoor-helper {HELPER_VERSION} starting"));

    #[cfg(target_os = "macos")]
    {
        let _ = fs::remove_file(SOCKET_PATH);
        if let Some(parent) = std::path::Path::new(SOCKET_PATH).parent() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let name = match build_socket_name() {
        Ok(n) => n,
        Err(e) => {
            log(&format!("socket name error: {e}"));
            std::process::exit(1);
        }
    };

    let listener = match ListenerOptions::new().name(name).create_sync() {
        Ok(l) => l,
        Err(e) => {
            log(&format!("bind failed: {e}"));
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o666)) {
            log(&format!("chmod failed: {e}"));
        }
    }

    log("listening");

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                std::thread::spawn(move || handle_client(stream));
            }
            Err(e) => {
                log(&format!("accept error: {e}"));
            }
        }
    }
}

fn build_socket_name() -> anyhow::Result<interprocess::local_socket::Name<'static>> {
    #[cfg(target_os = "windows")]
    {
        use interprocess::local_socket::GenericNamespaced;
        Ok(PIPE_NAME.to_ns_name::<GenericNamespaced>()?)
    }
    #[cfg(unix)]
    {
        use interprocess::local_socket::GenericFilePath;
        Ok(SOCKET_PATH.to_fs_name::<GenericFilePath>()?)
    }
}

// ---------- Windows: Service plumbing ----------

#[cfg(target_os = "windows")]
mod win_service {
    use super::*;
    use std::ffi::OsString;
    use std::sync::mpsc;
    use std::time::Duration;
    use windows_service::{
        define_windows_service,
        service::{
            ServiceAccess, ServiceControl, ServiceControlAccept, ServiceDependency, ServiceErrorControl,
            ServiceExitCode, ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult},
        service_dispatcher,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    define_windows_service!(ffi_service_main, service_main);

    pub fn run_dispatcher() -> windows_service::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)
    }

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_inner() {
            super::log(&format!("service error: {e}"));
        }
    }

    fn run_inner() -> windows_service::Result<()> {
        let (shutdown_tx, _shutdown_rx) = mpsc::channel::<()>();

        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let _ = shutdown_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        // Start the IPC server in a background thread; the service main thread waits for shutdown.
        std::thread::spawn(super::run_server);

        // Block until stop is requested
        let (_tx, rx) = mpsc::channel::<()>();
        let _ = rx.recv_timeout(Duration::from_secs(60 * 60 * 24 * 365));

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        Ok(())
    }

    pub fn install() -> windows_service::Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )?;

        let exe_path = std::env::current_exe()
            .expect("current_exe")
            .to_path_buf();

        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY_NAME),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe_path,
            launch_arguments: vec![OsString::from("--service")],
            dependencies: vec![ServiceDependency::Service(OsString::from("Tcpip"))],
            account_name: None, // LocalSystem
            account_password: None,
        };

        let service =
            manager.create_service(&info, ServiceAccess::START | ServiceAccess::CHANGE_CONFIG)?;
        let _ = service.set_description("CryptDoor TUN helper service");
        let _ = service.start::<&str>(&[]);
        Ok(())
    }

    pub fn uninstall() -> windows_service::Result<()> {
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT,
        )?;
        let service = manager.open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )?;
        let _ = service.stop();
        std::thread::sleep(std::time::Duration::from_millis(500));
        service.delete()?;
        Ok(())
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    {
        run_server();
    }

    #[cfg(target_os = "windows")]
    {
        let args: Vec<String> = std::env::args().collect();
        let mode = args.get(1).map(|s| s.as_str()).unwrap_or("");

        match mode {
            "install" => match win_service::install() {
                Ok(()) => {
                    log("service installed");
                    println!("CryptDoor helper service installed and started");
                }
                Err(e) => {
                    log(&format!("install failed: {e}"));
                    eprintln!("install failed: {e}");
                    std::process::exit(1);
                }
            },
            "uninstall" => match win_service::uninstall() {
                Ok(()) => {
                    log("service uninstalled");
                    println!("CryptDoor helper service uninstalled");
                }
                Err(e) => {
                    log(&format!("uninstall failed: {e}"));
                    eprintln!("uninstall failed: {e}");
                    std::process::exit(1);
                }
            },
            "--service" => {
                if let Err(e) = win_service::run_dispatcher() {
                    log(&format!("dispatcher error: {e}"));
                    std::process::exit(1);
                }
            }
            "--debug" => run_server(),
            _ => {
                eprintln!(
                    "usage: cryptdoor-helper [install|uninstall|--service|--debug]\n"
                );
                std::process::exit(2);
            }
        }
    }
}
