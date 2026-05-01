//! CryptDoor privileged helper daemon.
//!
//! Runs as root via launchd. Receives JSON line-delimited commands over a
//! Unix domain socket. Spawns mihomo with TUN privileges, kills it on stop.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

const SOCKET_PATH: &str = "/var/run/online.cryptdoor.helper.sock";
const HELPER_VERSION: &str = env!("CARGO_PKG_VERSION");
const LOG_PATH: &str = "/var/log/cryptdoor-helper.log";

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
    let line = format!(
        "[{}] {}\n",
        chrono_now(),
        msg
    );
    eprint!("{line}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let _ = f.write_all(line.as_bytes());
    }
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
        Request::Version => Response::ok(serde_json::json!({
            "version": HELPER_VERSION,
        })),
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

fn kill_stray_mihomo() {
    // Прибиваем чужие mihomo, которые могли остаться от прошлых dev-сборок
    // или от сломанных запусков, и держат TUN-устройство.
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

fn stop_inner() {
    let mut guard = CHILD.lock().expect("lock");
    if let Some(mut child) = guard.take() {
        log(&format!("killing mihomo pid={}", child.id()));
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn handle_client(stream: UnixStream) {
    let reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;

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
        if writer.write_all(&payload).is_err() {
            return;
        }
        let _ = writer.flush();
    }
}

fn main() {
    log(&format!("cryptdoor-helper {} starting", HELPER_VERSION));

    let _ = fs::remove_file(SOCKET_PATH);

    if let Some(parent) = std::path::Path::new(SOCKET_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            log(&format!("bind failed: {e}"));
            std::process::exit(1);
        }
    };

    if let Err(e) = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o666)) {
        log(&format!("chmod failed: {e}"));
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
