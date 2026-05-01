# CryptDoor

A simple, fast VPN client for macOS and Windows.

VLESS + REALITY over Mihomo. System-wide TUN mode: your IP actually changes — not just browser traffic.

## Install

### macOS (Apple Silicon — M1/M2/M3/M4)

1. Download the `.dmg` from the [Releases](https://github.com/AntonMhnk/cryptdoor.online/releases/latest) page.
2. Drag `CryptDoor.app` into `/Applications`.
3. Open it:
   ```bash
   xattr -cr /Applications/CryptDoor.app
   open /Applications/CryptDoor.app
   ```
4. On the first Connect, macOS will ask for your password — this installs the privileged TUN helper. You'll only need to do this once.

### Windows (10/11, 64-bit)

1. Download the `.exe` installer from the [Releases](https://github.com/AntonMhnk/cryptdoor.online/releases/latest) page.
2. Run it. If SmartScreen blocks the launch, click **More info** → **Run anyway**.
3. Install like any other app.
4. On the first Connect, Windows will show a UAC prompt — this registers the TUN helper service. You'll only need to do this once.

## Features

- **System TUN mode** — full traffic routing, your IP truly changes (Telegram, native apps, everything)
- **Multiple VLESS keys** with one-click switching
- **Tray icon** (menu bar / system tray) with quick connect / disconnect
- **Launch at login** — VPN is ready as soon as you log in
- **Silent auto-updates** — signed releases via GitHub, with a one-click "Install / Restart now" UI
- **Cross-platform** — same UX on macOS and Windows

## Stack

- **UI:** React 18, TypeScript, Vite
- **Native:** Tauri 2 (Rust)
- **VPN core:** Mihomo (MetaCubeX) as a sidecar
- **Transport:** VLESS + REALITY
- **TUN on Windows:** WinTun driver (from WireGuard)
- **TUN on macOS:** native `utun` interface

## Development

```bash
pnpm install
pnpm prebuild:mihomo            # download mihomo binary for the current platform
pnpm prebuild:wintun            # Windows targets only
pnpm build:helper               # build the privileged helper and stage it under sidecar/
pnpm tauri:dev
```

Local production build, signing the updater artifacts (reads the private key from `~/.tauri/cryptdoor.key`):

```bash
pnpm tauri:build:signed
```

## Releases

Releases are produced automatically by GitHub Actions for **macOS (Apple Silicon)** and **Windows x64** on every pushed tag. The workflow also generates `latest.json` for `tauri-plugin-updater`.

```bash
pnpm release:bump patch         # 0.1.0 → 0.1.1 (also: minor, major, or explicit version)
git commit -am "release v0.1.1"
git tag v0.1.1
git push && git push --tags
```

After ~15–20 minutes, the [Releases page](https://github.com/AntonMhnk/cryptdoor.online/releases) will have `.dmg`, `.exe`, `.app.tar.gz`, `.sig`, and `latest.json` attached.

### GitHub Actions secrets

Set under **Settings → Secrets and variables → Actions**:

| Name | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | The full contents of `~/.tauri/cryptdoor.key` |

The key was generated with an empty password, so `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is not required (GitHub doesn't allow empty secrets — the workflow defaults to an empty string).

Without `TAURI_SIGNING_PRIVATE_KEY`, the release workflow will **fail** while signing updater artifacts.

## Architecture

```
src/                            # React UI
├── App.tsx                     # main view, update banner, connection state
├── styles.css
└── lib/
    ├── vless.ts                # VLESS link parser + Mihomo YAML generator
    ├── tauri.ts                # invoke() wrapper
    └── storage.ts              # localStorage for keys

src-tauri/
├── src/
│   ├── lib.rs                  # Tauri runtime, tray icon, updater check
│   ├── commands.rs             # connect / disconnect / install_helper / install_update
│   ├── bin/helper.rs           # privileged daemon (launchd / Windows Service)
│   └── core/
│       ├── mihomo.rs           # spawn / supervise mihomo
│       ├── tun_config.rs       # platform-specific TUN YAML
│       └── helper_client.rs    # IPC client (Unix socket / Named Pipe)
├── sidecar/                    # mihomo + helper (populated by scripts/)
├── icons/                      # app and tray icons
├── windows/
│   └── hooks.nsh               # NSIS installer hooks (stop/start service)
├── resources/
│   └── online.cryptdoor.helper.plist  # macOS launchd plist
├── tauri.conf.json             # shared Tauri config
├── tauri.macos.conf.json       # macOS-specific overrides
└── tauri.windows.conf.json     # Windows-specific overrides
```

## How updates work

1. On every launch the app fetches `latest.json` from the latest GitHub release.
2. If a newer version exists, an **Update available** banner appears in the UI.
3. The user clicks **Install** — the app downloads the signed installer in the background, with a live progress bar.
4. Tauri verifies the minisign signature against the public key embedded in the binary.
5. The new bundle is staged. The user clicks **Restart now** to apply it (no surprise restarts).
6. On Windows, the NSIS installer's pre-install hook stops `CryptDoorHelper` so the helper binary can be replaced; the post-install hook starts it again. On macOS the new `.app` is swapped in place.

## License

Private project.
