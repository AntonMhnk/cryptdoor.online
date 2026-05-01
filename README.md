# CryptDoor

Secure, fast, simple VPN client.

VLESS + REALITY поверх Mihomo. Системный TUN-режим: меняется IP, а не просто трафик в браузере.

## Установка

### macOS (Apple Silicon, M1/M2/M3/M4)

1. Скачать `.dmg` со страницы [Releases](https://github.com/AntonMhnk/cryptdoor.online/releases/latest)
2. Перетащить `CryptDoor.app` в `/Applications`
3. Запустить:
   ```bash
   xattr -cr /Applications/CryptDoor.app
   open /Applications/CryptDoor.app
   ```
4. При первом подключении ввести пароль macOS — это нужно для установки TUN-помощника

### Windows (10/11, 64-bit)

1. Скачать `.exe`-установщик со страницы [Releases](https://github.com/AntonMhnk/cryptdoor.online/releases/latest)
2. Запустить — если SmartScreen ругается, нажать **More info** → **Run anyway**
3. Установить как обычное приложение
4. При первом подключении подтвердить UAC-запрос — это нужно для установки TUN-сервиса

## Возможности

- TUN-режим (системный, меняется IP)
- Поддержка нескольких VLESS-ключей с быстрым переключением
- Tray-иконка (menu bar) с быстрыми действиями
- Запуск при логине системы
- Автообновления (тихие, в фоне)

## Стек

- **UI:** React 18, TypeScript, Vite
- **Native:** Tauri 2 (Rust)
- **Core:** Mihomo (MetaCubeX) как sidecar
- **Транспорт:** VLESS + REALITY
- **TUN на Windows:** WinTun (Wireguard)

## Разработка

```bash
pnpm install
pnpm prebuild:mihomo      # скачать mihomo под текущую платформу
pnpm tauri:dev
```

## Релиз

Релизы собираются автоматически через GitHub Actions при пуше тега:

```bash
git tag v0.1.1
git push --tags
# через ~15 минут на странице Releases появятся .dmg + .exe
```

## Архитектура

```
src/                            # React UI
├── App.tsx
└── lib/
    ├── vless.ts                # парсер VLESS-ссылок и генератор YAML
    ├── tauri.ts                # invoke()-обёртка
    └── storage.ts              # localStorage для ключей

src-tauri/
├── src/
│   ├── lib.rs                  # Tauri runtime + tray icon
│   ├── commands.rs             # connect/disconnect/install_helper
│   ├── bin/helper.rs           # привилегированный демон
│   └── core/
│       ├── mihomo.rs           # запуск/остановка mihomo
│       ├── tun_config.rs       # YAML для TUN-режима
│       └── helper_client.rs    # IPC c helper'ом
├── sidecar/                    # mihomo + helper (генерируются скриптами)
├── icons/                      # иконки приложения и трея
└── resources/
    └── online.cryptdoor.helper.plist  # macOS launchd plist
```

## Лицензия

Private project.
