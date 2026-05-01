use anyhow::{anyhow, Context, Result};
use interprocess::local_socket::traits::Stream as _;
use interprocess::local_socket::{prelude::*, Name, Stream};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

#[cfg(target_os = "windows")]
use interprocess::local_socket::GenericNamespaced;

#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;

// ---------- platform constants ----------

#[cfg(target_os = "macos")]
pub const SOCKET_PATH: &str = "/var/run/online.cryptdoor.helper.sock";

#[cfg(target_os = "macos")]
pub const HELPER_BIN_PATH: &str = "/Library/PrivilegedHelperTools/cryptdoor-helper";

#[cfg(target_os = "macos")]
pub const PLIST_PATH: &str = "/Library/LaunchDaemons/online.cryptdoor.helper.plist";

#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub const HELPER_LABEL: &str = "online.cryptdoor.helper";

#[cfg(target_os = "windows")]
pub const PIPE_NAME: &str = "online.cryptdoor.helper";

#[cfg(target_os = "windows")]
pub const SERVICE_NAME: &str = "CryptDoorHelper";

// ---------- protocol ----------

#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Request {
    Version,
    Status,
    Start {
        mihomo: String,
        config: String,
        workdir: String,
    },
    Stop,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

// ---------- helper presence ----------

#[cfg(target_os = "macos")]
pub fn helper_installed() -> bool {
    use std::path::Path;
    Path::new(HELPER_BIN_PATH).exists() && Path::new(PLIST_PATH).exists()
}

#[cfg(target_os = "macos")]
pub fn helper_socket_ready() -> bool {
    use std::path::Path;
    Path::new(SOCKET_PATH).exists()
}

#[cfg(target_os = "windows")]
pub fn helper_installed() -> bool {
    use std::process::Command;
    let out = Command::new("sc").args(["query", SERVICE_NAME]).output();
    match out {
        Ok(o) => o.status.success() && String::from_utf8_lossy(&o.stdout).contains("SERVICE_NAME"),
        Err(_) => false,
    }
}

#[cfg(target_os = "windows")]
pub fn helper_socket_ready() -> bool {
    socket_name()
        .and_then(|n| Stream::connect(n).map_err(Into::into))
        .is_ok()
}

// ---------- socket name resolution ----------

pub fn socket_name() -> Result<Name<'static>> {
    #[cfg(target_os = "windows")]
    {
        Ok(PIPE_NAME.to_ns_name::<GenericNamespaced>()?)
    }
    #[cfg(unix)]
    {
        Ok(SOCKET_PATH.to_fs_name::<GenericFilePath>()?)
    }
}

// ---------- IPC ----------

pub fn send(req: Request) -> Result<Response> {
    let name = socket_name()?;
    let stream = Stream::connect(name).with_context(|| "connect to helper socket".to_string())?;

    let mut payload = serde_json::to_vec(&req)?;
    payload.push(b'\n');

    let mut writer = stream;
    writer.write_all(&payload)?;
    writer.flush()?;

    let mut reader = BufReader::new(writer);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.is_empty() {
        return Err(anyhow!("helper closed connection without response"));
    }
    let resp: Response = serde_json::from_str(&line)?;
    Ok(resp)
}

pub fn ping_with_retry(max_attempts: u32) -> Result<String> {
    let mut last_err = anyhow!("not attempted");
    for i in 0..max_attempts {
        match send(Request::Version) {
            Ok(r) => {
                if r.ok {
                    let v = r
                        .data
                        .and_then(|d| d.get("version").and_then(|v| v.as_str().map(String::from)))
                        .unwrap_or_default();
                    return Ok(v);
                }
                last_err = anyhow!("helper returned error: {:?}", r.error);
            }
            Err(e) => {
                last_err = e;
            }
        }
        std::thread::sleep(Duration::from_millis(200 + (i as u64) * 100));
    }
    Err(last_err)
}
