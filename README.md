# AC Remote

A tiny tray / menu-bar app for Windows and macOS that controls a **Midea Wi-Fi air conditioner**
(tested target: Midea Xtreme Plus Inverter) **directly over your local network** — no phone, no browser.

## Stack
- [Tauri v2](https://tauri.app) — Rust backend + native webview, small installers
- React + TypeScript + Vite for the popup UI
- A Rust port of the Midea LAN (V3 / "8370") protocol, based on the
  [msmart-ng](https://github.com/mill1000/midea-msmart) reference implementation

## Development
Prerequisites: Node 20+, Rust (stable), and on Windows the MSVC build tools + WebView2.

```sh
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # produce installers in src-tauri/target/release/bundle
```
