use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

pub const SOCKET_PATH: &str = "/var/run/online.cryptdoor.helper.sock";
pub const HELPER_BIN_PATH: &str = "/Library/PrivilegedHelperTools/cryptdoor-helper";
pub const PLIST_PATH: &str = "/Library/LaunchDaemons/online.cryptdoor.helper.plist";
pub const HELPER_LABEL: &str = "online.cryptdoor.helper";

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

pub fn helper_installed() -> bool {
    Path::new(HELPER_BIN_PATH).exists() && Path::new(PLIST_PATH).exists()
}

pub fn helper_socket_ready() -> bool {
    Path::new(SOCKET_PATH).exists()
}

pub fn send(req: Request) -> Result<Response> {
    let stream = UnixStream::connect(SOCKET_PATH)
        .with_context(|| format!("connect to helper socket {SOCKET_PATH}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(8)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let mut writer = stream.try_clone()?;
    let payload = serde_json::to_vec(&req)?;
    writer.write_all(&payload)?;
    writer.write_all(b"\n")?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
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
