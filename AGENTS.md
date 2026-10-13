# Rikette desktop

This repository is generated from the private rikette.tv monorepo (`scripts/publish-desktop.mjs` there) on every change: `apps/desktop`, `packages/` and the root build files. Don't push here directly; changes land in the monorepo and come back with the next sync. Releases (installer, `.sig`, `latest.json`) are published here too, tagged on the source they were built from.

# UI

The app shares one theme with the website: Instant replay, TV sports replay graphics on deep navy (background `#070b15`, panels `#0c1220`, one accent = replay blue `#2f6bff`, the logo red `#ff2d3f` for live/REPLAY/now; condensed Archivo titles, Martian Mono timecodes; square corners, no gradients, blur, glows or pills). See `design.md`, including its "Never" list.

All colours live in `:root` of `apps/desktop/src/renderer/src/styles/globals.css` (plus copies in `overlay.html` and `splash.html`, which have no Tailwind). Use semantic tokens (`bg-background`, `text-muted-foreground`, `bg-primary`, `bg-rec`, `bg-accent`, `bg-[var(--scrim)]`) — no one-off hex, no `white/…` or `black/…` tints for chrome. Buttons are flat blocks: `Button` default = blue `.btn`, `secondary`/`outline` = outlined `.btn btn-secondary`; cards use `.card`; TV tags use `.bug`; titles use `PageHeader` (`display-title`/`eyebrow`); times and numbers use `.tc`; hotkeys use `Kbd`. Canvas code reads tokens with `getComputedStyle`. Navigation and the window buttons are the top bar in `TopNav.tsx`.

## Form controls

Never use native `<select>`, `<input type="checkbox">`, `<input type="radio">`, or `<input type="range">`. Add or reuse shadcn components under `apps/desktop/src/renderer/src/components/ui` (`Select`, `Switch`, `Slider`, `Input`, `Progress`, `ToggleGroup`). If a primitive is missing, add it there first.

`pnpm dlx shadcn add …` must be run from `apps/desktop`. If files land in `apps/desktop/@/`, move them into `src/renderer/src/components/ui` and import `cn` from `@/lib/utils`, not the `cn` npm package.

# Desktop app

`apps/desktop` is a Tauri 2 app: a Rust shell around the React renderer, with recording in its own process.

- `src/renderer/` is the UI. It only talks to the app through `window.echoframe` (`EchoFrameApi` in `src/tauri/api.ts`), which `src/tauri/bridge.ts` implements with the commands in `src-tauri/src/ipc.rs` and their events. A new API method needs a command there and a line in the bridge.
- `src-tauri/` is the shell (crate `rikette`): settings, the clip library (`library.db`, SQLite), saving and trims (`clips.rs`, `media/ffmpeg.rs`), uploads, auth, tray, hotkey, the save HUD (`overlay.rs`), splash, updater and `rikette://` links. Media reaches the page through the `efmedia` protocol (`media_protocol.rs`, byte ranges for seeking).
- `capture/` is `rikette-capture`, the process that owns libobs (through the `libobs` FFI crate from libobs-rs), so a crash in OBS or a game hook never takes the app down. `capture-protocol/` defines the JSON-lines messages on its stdin/stdout. In the shell, `src/capture/remote.rs` spawns and drives it, and `src/capture/engine.rs` owns status, pause, save single-flight and restarts (including bounded auto-restart when the capture process dies). In `capture/`: `obs/mod.rs` (startup, video, replay buffer and encoder fallback, save), `obs/scene.rs` (game/display/window sources, auto game capture), `obs/audio.rs` (desktop/mic sources), `encoders.rs` (OBS encoder ids and settings), and `win/` for Windows APIs (fullscreen game detection, WASAPI devices, headset volume).
- `scripts/obs-runtime.mjs` (Windows) fills `obs-runtime/` with only the OBS files Rikette uses, `ffmpeg.exe`/`ffprobe.exe` from the matching obs-deps (everything after a save uses them), the VC++ runtime and `rikette-capture.exe`; the installer ships it as `obs/`. Bumping OBS means bumping the `libobs` crate pin and the hashes in that script together.
- Data lives in `%APPDATA%\Rikette` (`settings.json`, `library.db`, `thumbs/`, logs). App logs go to stdout and `logs\main.log`; lines from the capture process start with `[capture]`, OBS's own with `[obs]`. The app version is `apps/desktop/package.json`'s.
- `pnpm --filter desktop dev` runs it (run `pnpm --filter desktop obs-runtime --debug` first on Windows; elsewhere capture reports it is unavailable and the rest runs).

`pnpm lint` must stay at zero problems and `pnpm typecheck` clean; `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` (`pnpm test`) in `apps/desktop` must pass. CI runs them on every push. The app links libobs-rs (GPL-3.0), so it is GPL-3.0-or-later.
