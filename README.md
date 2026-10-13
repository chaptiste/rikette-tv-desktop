# Rikette desktop

The Windows app behind [rikette.tv](https://rikette.tv): a replay buffer recorder for games. It keeps the last seconds of your game in memory, and one hotkey (Alt+1 by default) saves them as a clip, without leaving the game.

- **Built on OBS.** Recording uses libobs (through [libobs-rs](https://github.com/libobs-rs/libobs-rs)) with your GPU's encoder (NVENC, AMF or QuickSync, x264 as a fallback), in its own process so a crash in a game hook never takes the app down.
- **Written in Rust.** A [Tauri 2](https://tauri.app) shell around a React UI, using the system WebView2 instead of a bundled browser.
- **Light.** Only the parts of OBS Rikette uses are shipped, and the window's webview is freed while the app sits in the tray.

Download the installer from [rikette.tv](https://rikette.tv) or the [releases](https://github.com/chaptiste/rikette-tv-desktop/releases).

## Layout

```
apps/desktop/
  src/renderer/        the UI (React, Tailwind)
  src/tauri/           window.echoframe, the API the UI uses, backed by Tauri commands
  src-tauri/           the app shell (crate `rikette`): settings, clip library, saving, trims, tray, updater
  capture/             rikette-capture, the process that drives libobs
  capture-protocol/    the JSON-lines messages between the two
  scripts/obs-runtime.mjs   fetches OBS and keeps only the files Rikette uses
packages/shared/       constants and types shared with the rikette.tv website
```

`AGENTS.md` describes the architecture in more detail, and `design.md` the look.

## Building

On Windows, with the Visual Studio 2022 Build Tools ("Desktop development with C++"), Rust stable, Node 22 and pnpm 10 (`corepack enable`):

```powershell
pnpm install
pnpm --filter desktop obs-runtime --debug   # downloads OBS and its deps once, builds rikette-capture
pnpm --filter desktop dev
```

On macOS and Linux the app runs, but capture reports that it is unavailable.

Checks: `pnpm typecheck`, `pnpm lint`, and in `apps/desktop`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.

## Contributing

This repository is published from the private rikette.tv monorepo on every change, so its history is generated. Issues and pull requests are welcome here; accepted changes are applied upstream and come back with the next sync.

## License

GPL-3.0-or-later (see `LICENSE`). Rikette links libobs (GPL-2.0-or-later) and libobs-rs (GPL-3.0).
