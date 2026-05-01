use anyhow::{anyhow, Context, Result};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[derive(Default)]
struct State {
    child: Option<Child>,
    config_path: Option<PathBuf>,
}

static STATE: OnceCell<Arc<Mutex<State>>> = OnceCell::new();
static APP: OnceCell<AppHandle> = OnceCell::new();

pub fn init(handle: AppHandle) -> Result<()> {
    STATE.get_or_init(|| Arc::new(Mutex::new(State::default())));
    APP.set(handle).map_err(|_| anyhow!("AppHandle already set"))?;
    Ok(())
}

fn state() -> Arc<Mutex<State>> {
    STATE
        .get_or_init(|| Arc::new(Mutex::new(State::default())))
        .clone()
}

fn app_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow!("no data dir"))?
        .join("CryptDoor");
    std::fs::create_dir_all(&dir).context("create data dir")?;
    Ok(dir)
}

fn mihomo_binary() -> Result<PathBuf> {
    let app = APP.get().ok_or_else(|| anyhow!("app handle not set"))?;
    let resolver = app.path();
    if let Ok(resource_path) = resolver.resolve("sidecar/mihomo", tauri::path::BaseDirectory::Resource) {
        if resource_path.exists() {
            return Ok(resource_path);
        }
    }
    let manifest = std::env::current_exe()?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("no parent dir"))?;
    let candidate = manifest.join("mihomo");
    if candidate.exists() {
        return Ok(candidate);
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("sidecar")
        .join(if cfg!(windows) { "mihomo.exe" } else { "mihomo" });
    if dev.exists() {
        return Ok(dev);
    }
    Err(anyhow!(
        "mihomo binary not found (tried resource and {:?})",
        dev
    ))
}

pub fn start(config_yaml: &str) -> Result<u16> {
    let st = state();
    {
        let mut s = st.lock();
        if let Some(child) = s.child.as_mut() {
            if let Ok(None) = child.try_wait() {
                return Ok(default_mixed_port());
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        s.child = None;
    }

    let data = app_data_dir()?;
    let cfg_path = data.join("config.yaml");
    std::fs::write(&cfg_path, config_yaml).context("write config.yaml")?;

    let bin = mihomo_binary()?;
    log::info!("starting mihomo: {:?} -d {:?}", bin, data);

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data.join("mihomo.log"))?;
    let log_err = log_file.try_clone()?;

    let child = Command::new(&bin)
        .arg("-d")
        .arg(&data)
        .arg("-f")
        .arg(&cfg_path)
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err))
        .spawn()
        .context("spawn mihomo")?;

    {
        let mut s = st.lock();
        s.child = Some(child);
        s.config_path = Some(cfg_path);
    }

    Ok(default_mixed_port())
}

pub fn stop() -> Result<()> {
    let st = state();
    let mut s = st.lock();
    if let Some(mut child) = s.child.take() {
        log::info!("stopping mihomo pid={}", child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

pub fn stop_blocking() -> Result<()> {
    stop()
}

pub fn is_running() -> bool {
    let st = state();
    let mut s = st.lock();
    if let Some(child) = s.child.as_mut() {
        match child.try_wait() {
            Ok(None) => true,
            _ => {
                s.child = None;
                false
            }
        }
    } else {
        false
    }
}

pub const fn default_mixed_port() -> u16 {
    7899
}
