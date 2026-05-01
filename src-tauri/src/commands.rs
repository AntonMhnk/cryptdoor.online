use crate::core::helper_client::{self, Request as HelperRequest};
use crate::core::mihomo;
use crate::core::tun_config;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Serialize)]
pub struct ConnectionStatus {
    pub running: bool,
    pub port: u16,
    pub mode: String,
    pub helper_installed: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConnectArgs {
    pub config_yaml: String,
}

fn fail<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn fail_chain(e: anyhow::Error) -> String {
    format!("{e:#}")
}

fn workdir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .ok_or_else(|| anyhow!("no data dir"))?
        .join("CryptDoor");
    std::fs::create_dir_all(&dir).context("create workdir")?;
    Ok(dir)
}

fn resolve_mihomo(_app: &AppHandle) -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let parent = exe
        .parent()
        .ok_or_else(|| anyhow!("no parent for current_exe"))?;

    let triple = current_triple();
    let bin_name = if cfg!(target_os = "windows") {
        "mihomo.exe"
    } else {
        "mihomo"
    };
    let triple_name = if cfg!(target_os = "windows") {
        format!("mihomo-{triple}.exe")
    } else {
        format!("mihomo-{triple}")
    };

    let candidates = [
        parent.join(bin_name),
        parent.join(&triple_name),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("sidecar")
            .join(&triple_name),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("sidecar")
            .join(bin_name),
    ];

    for c in candidates.iter() {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    Err(anyhow!(
        "mihomo binary not found. tried: {:?}",
        candidates
    ))
}

fn current_triple() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
}

#[tauri::command]
pub async fn helper_status() -> Result<serde_json::Value, String> {
    let installed = helper_client::helper_installed();
    let socket_ready = helper_client::helper_socket_ready();
    let mut version = None;
    if socket_ready {
        if let Ok(v) = tokio::task::spawn_blocking(|| helper_client::ping_with_retry(3))
            .await
            .map_err(fail)?
        {
            version = Some(v);
        }
    }
    Ok(serde_json::json!({
        "installed": installed,
        "socketReady": socket_ready,
        "version": version,
    }))
}

#[tauri::command]
pub async fn install_helper(app: AppHandle) -> Result<(), String> {
    install_helper_inner(&app).await.map_err(fail_chain)
}

async fn install_helper_inner(app: &AppHandle) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        install_helper_macos(app).await
    }
    #[cfg(target_os = "windows")]
    {
        install_helper_windows(app).await
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err(anyhow!("unsupported platform"))
    }
}

#[cfg(target_os = "macos")]
async fn install_helper_macos(app: &AppHandle) -> Result<()> {
    let resolver = app.path();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let triple = current_triple();
    let helper_candidates: Vec<PathBuf> = vec![
        // Production: bundled via externalBin → next to CryptDoor binary
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("cryptdoor-helper")))
            .unwrap_or_default(),
        // Dev: cargo build output staged by copy-helper.mjs (with triple suffix)
        manifest_dir
            .join("sidecar")
            .join(format!("cryptdoor-helper-{triple}")),
        manifest_dir.join("sidecar").join("cryptdoor-helper"),
        resolver
            .resolve("cryptdoor-helper", tauri::path::BaseDirectory::Resource)
            .unwrap_or_default(),
    ];
    let helper_src = helper_candidates
        .into_iter()
        .find(|p| !p.as_os_str().is_empty() && p.exists())
        .ok_or_else(|| {
            anyhow!(
                "helper binary not found. Build it: cd src-tauri && cargo build --bin cryptdoor-helper && cp target/debug/cryptdoor-helper sidecar/"
            )
        })?;

    let plist_candidates: Vec<PathBuf> = vec![
        manifest_dir
            .join("resources")
            .join("online.cryptdoor.helper.plist"),
        resolver
            .resolve(
                "online.cryptdoor.helper.plist",
                tauri::path::BaseDirectory::Resource,
            )
            .unwrap_or_default(),
    ];
    let plist_src = plist_candidates
        .into_iter()
        .find(|p| !p.as_os_str().is_empty() && p.exists())
        .ok_or_else(|| anyhow!("helper plist not found"))?;

    let staging = std::env::temp_dir().join(format!("cryptdoor-install-{}", std::process::id()));
    std::fs::create_dir_all(&staging).context("create staging dir")?;

    let staged_helper = staging.join("cryptdoor-helper");
    let staged_plist = staging.join("online.cryptdoor.helper.plist");
    std::fs::copy(&helper_src, &staged_helper).context("copy helper to staging")?;
    std::fs::copy(&plist_src, &staged_plist).context("copy plist to staging")?;

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&staged_helper, std::fs::Permissions::from_mode(0o755))?;
    std::fs::set_permissions(&staged_plist, std::fs::Permissions::from_mode(0o644))?;

    let staged_helper_str = staged_helper.to_string_lossy().to_string();
    let staged_plist_str = staged_plist.to_string_lossy().to_string();

    let script = format!(
        r#"#!/bin/bash
set -e
cd /
mkdir -p /Library/PrivilegedHelperTools
cp '{helper_src}' /Library/PrivilegedHelperTools/cryptdoor-helper
chown root:wheel /Library/PrivilegedHelperTools/cryptdoor-helper
chmod 755 /Library/PrivilegedHelperTools/cryptdoor-helper
xattr -d -r com.apple.quarantine /Library/PrivilegedHelperTools/cryptdoor-helper 2>/dev/null || true
cp '{plist_src}' /Library/LaunchDaemons/online.cryptdoor.helper.plist
chown root:wheel /Library/LaunchDaemons/online.cryptdoor.helper.plist
chmod 644 /Library/LaunchDaemons/online.cryptdoor.helper.plist
launchctl unload /Library/LaunchDaemons/online.cryptdoor.helper.plist 2>/dev/null || true
launchctl load -w /Library/LaunchDaemons/online.cryptdoor.helper.plist
"#,
        helper_src = staged_helper_str.replace('\'', "'\\''"),
        plist_src = staged_plist_str.replace('\'', "'\\''"),
    );

    let tmp = staging.join("install.sh");
    std::fs::write(&tmp, script)?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;

    let tmp_str = tmp.to_string_lossy().replace('"', "\\\"");
    let osa = format!(
        r#"do shell script "cd / && /bin/bash '{tmp_str}'" with prompt "CryptDoor needs to install a VPN component" with administrator privileges"#
    );

    let output = tokio::process::Command::new("osascript")
        .args(["-e", &osa])
        .current_dir("/")
        .output()
        .await?;

    let _ = std::fs::remove_dir_all(&staging);

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("install cancelled or failed: {}", err.trim()));
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    tokio::task::spawn_blocking(|| helper_client::ping_with_retry(20))
        .await
        .map_err(|e| anyhow!("join error: {e}"))?
        .context("helper not responding after install")?;

    Ok(())
}

#[cfg(target_os = "windows")]
async fn install_helper_windows(_app: &AppHandle) -> Result<()> {
    let exe = std::env::current_exe()?;
    let parent = exe
        .parent()
        .ok_or_else(|| anyhow!("no parent dir for current_exe"))?;
    let helper_exe = parent.join("cryptdoor-helper.exe");
    if !helper_exe.exists() {
        return Err(anyhow!(
            "cryptdoor-helper.exe not found at {}",
            helper_exe.display()
        ));
    }

    let helper_str = helper_exe.to_string_lossy().to_string();

    // Запрашиваем UAC через PowerShell + Start-Process -Verb RunAs.
    // -Wait блокирует пока пользователь не закроет UAC-диалог и helper не закончит install.
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'; $p = Start-Process -FilePath "{path}" -ArgumentList "install" -Verb RunAs -Wait -PassThru; if ($p.ExitCode -ne 0) {{ exit $p.ExitCode }}"#,
        path = helper_str.replace('"', "`\"")
    );

    let output = tokio::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .await
        .context("spawn powershell")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        // Helper exits 72 when the service is in MARKED_FOR_DELETE state — only
        // a Windows reboot can clear that. Surface a friendlier hint in the UI.
        if code == 72 {
            return Err(anyhow!(
                "Old CryptDoor service is still being removed by Windows. \
                 Please reboot Windows once and try Connect again."
            ));
        }
        return Err(anyhow!(
            "UAC install cancelled or failed (exit {}): {}",
            code,
            err.trim()
        ));
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    tokio::task::spawn_blocking(|| helper_client::ping_with_retry(40))
        .await
        .map_err(|e| anyhow!("join error: {e}"))?
        .context("helper service not responding after install")?;

    Ok(())
}

#[tauri::command]
pub async fn connect_proxy(
    app: AppHandle,
    args: ConnectArgs,
) -> Result<ConnectionStatus, String> {
    connect_inner(&app, args.config_yaml)
        .await
        .map_err(fail_chain)
}

async fn connect_inner(app: &AppHandle, user_yaml: String) -> Result<ConnectionStatus> {
    if !helper_client::helper_installed() {
        install_helper_inner(app)
            .await
            .context("installing TUN component")?;
    }

    helper_client::ping_with_retry(30).context("helper not responding")?;

    let mihomo_path = resolve_mihomo(app)?;
    let workdir = workdir()?;
    let yaml_with_tun = tun_config::wrap_with_tun(&user_yaml)?;

    let req = HelperRequest::Start {
        mihomo: mihomo_path.to_string_lossy().to_string(),
        config: yaml_with_tun,
        workdir: workdir.to_string_lossy().to_string(),
    };

    let resp = tokio::task::spawn_blocking(move || helper_client::send(req))
        .await
        .map_err(|e| anyhow!("join error: {e}"))?
        .context("send Start to helper")?;

    if !resp.ok {
        return Err(anyhow!(
            "helper returned error: {}",
            resp.error.unwrap_or_default()
        ));
    }

    let port = mihomo::default_mixed_port();
    if let Err(e) = wait_for_port(port, Duration::from_secs(12)).await {
        let _ = tokio::task::spawn_blocking(|| helper_client::send(HelperRequest::Stop)).await;
        return Err(e);
    }

    #[cfg(target_os = "macos")]
    let _ = disable_ipv6_macos().await;

    Ok(ConnectionStatus {
        running: true,
        port,
        mode: "tun".into(),
        helper_installed: true,
    })
}

#[cfg(target_os = "macos")]
async fn list_network_services() -> Result<Vec<String>> {
    let out = tokio::process::Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .map(|l| l.trim().trim_start_matches('*').trim().to_string())
        .filter(|s| !s.is_empty() && !s.contains('('))
        .collect())
}

#[cfg(target_os = "macos")]
async fn disable_ipv6_macos() -> Result<()> {
    let services = list_network_services().await.unwrap_or_default();
    for svc in services {
        let _ = tokio::process::Command::new("networksetup")
            .args(["-setv6off", &svc])
            .output()
            .await;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
async fn restore_ipv6_macos() -> Result<()> {
    let services = list_network_services().await.unwrap_or_default();
    for svc in services {
        let _ = tokio::process::Command::new("networksetup")
            .args(["-setv6automatic", &svc])
            .output()
            .await;
    }
    Ok(())
}

async fn wait_for_port(port: u16, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            Ok(_) => return Ok(()),
            Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
    Err(anyhow!(
        "mihomo didn't open port {} within {:?} — check your VLESS key",
        port,
        timeout
    ))
}

#[tauri::command]
pub async fn disconnect_proxy() -> Result<ConnectionStatus, String> {
    if helper_client::helper_socket_ready() {
        let _ = tokio::task::spawn_blocking(|| helper_client::send(HelperRequest::Stop)).await;
    }
    let _ = tokio::task::spawn_blocking(mihomo::stop).await;
    Ok(ConnectionStatus {
        running: false,
        port: mihomo::default_mixed_port(),
        mode: "off".into(),
        helper_installed: helper_client::helper_installed(),
    })
}

#[tauri::command]
pub async fn connection_status() -> Result<ConnectionStatus, String> {
    let helper_installed = helper_client::helper_installed();
    let mut running = false;
    if helper_installed && helper_client::helper_socket_ready() {
        if let Ok(Ok(resp)) = tokio::task::spawn_blocking(|| {
            helper_client::send(HelperRequest::Status)
        })
        .await
        {
            if resp.ok {
                running = resp
                    .data
                    .and_then(|d| d.get("running").and_then(|r| r.as_bool()))
                    .unwrap_or(false);
            }
        }
    }
    Ok(ConnectionStatus {
        running,
        port: mihomo::default_mixed_port(),
        mode: if running { "tun".into() } else { "off".into() },
        helper_installed,
    })
}

#[tauri::command]
pub async fn window_show<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.show().map_err(fail)?;
        win.set_focus().map_err(fail)?;
    }
    Ok(())
}

#[tauri::command]
pub fn tray_set_status<R: Runtime>(
    app: AppHandle<R>,
    label: String,
    connected: bool,
) -> Result<(), String> {
    let menu = crate::build_menu(&app, connected).map_err(fail)?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu)).map_err(fail)?;
        tray.set_icon(Some(crate::tray_image(connected)))
            .map_err(fail)?;
        let tooltip = if connected {
            format!("CryptDoor — {label}")
        } else {
            "CryptDoor".to_string()
        };
        tray.set_tooltip(Some(&tooltip)).map_err(fail)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn current_external_ip() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(fail)?;
    let txt = client
        .get("https://api.ipify.org")
        .send()
        .await
        .map_err(fail)?
        .text()
        .await
        .map_err(fail)?;
    Ok(txt.trim().to_string())
}

#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|e| format!("updater init: {e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("update check: {e}"))?
        .ok_or_else(|| "no update available".to_string())?;

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("download/install: {e}"))?;

    app.restart();
}
