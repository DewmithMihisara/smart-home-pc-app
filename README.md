# AC Remote

A small tray (Windows) / menu-bar (macOS) app that controls **Midea Wi-Fi air conditioners**
— built for the Midea Xtreme Plus Inverter — **directly over your local network**.
No phone, no browser, no cloud round-trips.

Click the tray icon → a compact popup opens next to it:

- power, target temperature (16–30 °C in 0.5° steps), indoor/outdoor temperature
- mode: Auto / Cool / Dry / Heat / Fan · fan: Auto / Low / Med / High
- Swing, Eco and Turbo
- multiple ACs (switch from the header), launch at login, tray tooltip with the current state
- right-click the tray icon for *Toggle power* without opening the popup

## First-time setup

1. Start AC Remote — the setup screen opens.
2. **Scan network** and pick your air conditioner (or type its IP if the scan finds nothing).
3. Sign in with the **Midea SmartHome account the AC is registered to**.
   This is used **once** to download the AC's local encryption key (newer "V3" units require it).
   Your password is not stored; only the device's token/key are saved on this computer.
   Already have a token & key (e.g. from `msmart-ng discover`)? Choose *I already have a token & key*.
4. Done — from now on everything happens on your LAN.

> **Windows firewall:** the first scan may trigger a firewall prompt. Allow access on private
> networks, otherwise discovery replies (UDP) are blocked. You can always add a device by IP.

## Install

Grab an installer from the GitHub Actions artifacts (or build it yourself, below):

- **Windows:** `AC Remote_x.y.z_x64-setup.exe` (per-user install, no admin needed)
- **macOS:** `AC Remote_x.y.z_universal.dmg`. The app isn't notarized, so the first time
  right-click → **Open**, or run `xattr -dr com.apple.quarantine "/Applications/AC Remote.app"`.

## How it works

```
React popup  ──invoke/events──▶  Rust (Tauri)
                                   ├─ discover.rs   UDP broadcast on 6445/20086
                                   ├─ lan.rs        TCP 6444, V3 handshake (token → session key)
                                   ├─ packet.rs     0x8370 (AES-256-CBC+SHA256) ⟶ 0x5A5A (AES-ECB+MD5) ⟶ 0xAA frame
                                   ├─ ac.rs         0x41 query / 0x40 set state / 0xC0 state response
                                   └─ cloud.rs      one-time SmartHome login + getToken
```

The protocol is a Rust port of [msmart-ng](https://github.com/mill1000/midea-msmart) (MIT);
unit tests reuse its test vectors, plus an end-to-end test against a mock V3 device.

Saved devices live in the app's config dir (`config.json`, via `tauri-plugin-store`).

## Development

Prerequisites: Node 20+, Rust stable, and on Windows the MSVC build tools + WebView2 (preinstalled on Windows 11).

```sh
npm install
npm run tauri dev                         # run with hot reload
cd src-tauri && cargo test --lib          # protocol tests
cargo run --example discover              # list Midea devices on the LAN
MIDEA_EMAIL=... MIDEA_PASSWORD=... cargo run --example probe   # fetch key + read state (read-only)
npm run tauri build                       # installers → src-tauri/target/release/bundle/
```

macOS builds must be made on a Mac (`npm run tauri build -- --target universal-apple-darwin`)
or by the included GitHub Actions workflow (`.github/workflows/build.yml`, runs on `v*` tags or manually).
