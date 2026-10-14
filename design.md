# Design — Rikette.tv

Locked design system. Future runs read this file first; pages defer to it.
Amend intentionally — the file is the rule.

One theme on both surfaces: **Instant replay**. Rikette is a replay buffer, so the look comes from TV sports replay graphics: deep navy, one bright blue, the red dot from the logo for anything live, condensed heavy titles, tabular-figure timecodes, square corners. The signature element is the **buffer strip**: one cell per second of the buffer, ending at a red "now" line.
- **Website** (`apps/web`, every page and email). Source of truth: `apps/web/src/theme.ts`.
- **Desktop app** (`apps/desktop`). Source of truth: `:root` in `apps/desktop/src/renderer/src/styles/globals.css` (copies in `overlay.html` and `splash.html`, which have no Tailwind). Keep its hex values in step with `PALETTE`.

## Never
Blurred colour blobs, gradients (fills, text or backgrounds; the only gradient allowed is a scrim behind text laid over footage), glows, glassmorphism/backdrop blur, pill-shaped buttons, rounded cards, bouncy easing, icon-in-a-tinted-circle feature cards, stat-card rows. If it looks like a generic SaaS template, it is wrong.

## Palette
- Background `#070b15` · panel/card `#0c1220` · popover/raised `#111a2c` · input `#0a101d` · secondary fill `#141d31` · border `#1c2740` · border-strong `#2a3858` · text `#e9edf6` · muted `#7f8bab` (web `#8692b0`).
- Replay blue `#2f6bff` (hover `#4f82ff`, deep `#2458e0`, white ink): the one accent. Filled buttons, the active tab rule, selection, the buffer cells, "new" markers.
- Light blue `#7fa6ff`: links and the ten-second markers in the strip only.
- Red `#ff2d3f` (the logo dot): REC light, REPLAY tags, the "now" line and the editor playhead. Never for buttons except destructive confirms (`#ff4d5e`).
- Status: ok `#3ddc97`, warn/paused `#ffb547`.
- Text over footage sits on `--scrim` (`rgb(7 11 21 / 82%)`).

## Type
- **Archivo** (variable, width axis) for everything. Titles: 800 weight at 72 % width (`display-title`), tight leading. Button labels and small titles: 700 at ~85 % width (`font-display`). Body: 100 % width, 400.
- Timecodes, sizes, units, tab numbers and small uppercase labels (`eyebrow`, `tc`) are Archivo at 100 % width with tabular figures; no monospace font. Uppercase is only for these small labels and tags.
- The app bundles it via `@fontsource-variable/archivo`; the website loads it from Google Fonts in `page()`; emails use the system stack.

## Logo
A square navy tile (no rounded corners) with a condensed off-white R, the red REC dot top-right, and a small buffer strip under it: five blue cells that brighten toward the right and end at a red "now" line. The source is `apps/desktop/build/logo.svg`. `node scripts/build-logo.mjs` renders every PNG/ICO/ICNS from it (app icon, tray, splash, in-app logo, website favicon and email logo), so edit the SVG and never the PNGs.

## Components
- **Buttons** · flat blocks, radius 3 px, condensed bold label. Default = solid blue; `secondary`/`outline` = transparent with a 1 px `border-strong` outline; `.btn-danger` = red. Hover lightens, press drops 1 px. No shadows. App: `Button` (`.btn`, `.btn-secondary`, `.btn-danger`); web `.btn`, `.btn-dark`, `.btn-sm`, `.btn-lg` (a `<small>` inside a button is a small caption).
- **Cards** · flat `--card` panels with a 1 px border and 3 px radius. Emphasis is a 2 px blue top border (dialogs, featured tier, sign-in panels) or a 3 px blue left bar (selected rows, call-outs) — never a shadow or glow.
- **Bug** (`.bug`) · the TV replay tag: a red block ("REPLAY") next to a dark small timecode block. `.bug-blue` swaps the red for blue.
- **Lower third** · a blue block (number or duration) next to a raised panel with a small kicker and a condensed title, a 2 px blue underline; it wipes in from the left. Used for "Clip saved" in the app, the in-game overlay and the landing page.
- **Key caps** · small small uppercase caps with a 2 px bottom border (`Kbd` → `.kbd-key`; web `.kbd`). On a blue button use `.kbd-on-primary`.
- **Form controls** · shadcn primitives: square-cornered inputs/selects with a blue focus border, rectangular switches with a square knob, a thin slider track with a bar thumb, a flat blue progress bar, segmented toggle groups, small small badges.
- Emails: card with a blue top border, square buttons with a darker 2 px bottom edge.

## Desktop app layout
- **Top bar** (`TopNav.tsx`) · logo + "Rikette" in a cell with a right border, then text tabs (Replay, Clips, Editor, Settings, Profile), each followed by its small Ctrl number; a 2 px blue rule slides under the active tab. The REC light (`RecChip.tsx`: REC / SAVING / STARTING / PAUSED / OFFLINE, blinking red dot, buffer seconds) sits on the right. The Windows title-bar overlay is 44 px tall and `#070b15`.
- **PageHeader** · small kicker + a 40 px condensed title, actions on the right. Without a title it is a toolbar row (the editor).
- **Replay** (`/`) · left: the state title ("Rolling." / "Paused." / "Warming up." / "Cutting…" / "Off air.") with a muted subline and the encoder/source readout; the **last replay** (newest clip thumbnail framed with a REPLAY bug, game tag and lower-third title; click opens the editor; a white frame flash while saving; a "Clip saved" lower third when a new clip lands); the **buffer strip** (`BufferRuler`: up to 60 cells fill from the "now" end after a start using `CaptureStatus.bufferedSince`, older cells dimmer, a light-blue marker every ten seconds slides toward the past once full, a red now line, small "seconds ago" ticks); then the `xl` "Clip it" button with the hotkey caps, Pause/Resume (Restart when offline) and a small "change hotkey" link. Right: "Latest clips" as a race timing tower (position number, thumbnail, title, small meta; clips under two minutes old get the blue bar) and a 2×2 readout (buffer, quality, encoder, game/source).
- **Clips** (`/library`) · thumbnail grid with square corners; status badge top-left, game name and small duration over a bottom scrim; hover = 2 px blue outline and a blue play square; upload/link and menu chips on hover.
- **Editor** · square video with a hairline outline, small meta chips; timeline in a card with a blue selection and square handles; the playhead is red. Audio lanes: desktop light blue, mic `--ok`, other muted; their playhead is red too. Times use tabular figures.
- **Settings** · numbered rundown list on the left (01 Capture … 05 System, active row = card fill + blue left bar); the panel shows the section title, "01 / 05" and rows separated by hairlines.
- **Profile** (`/account`) · square blue initial block, small plan tag, storage bar; tier cards with a 48 px condensed price, the featured one with a blue top border and a green "SAVE x%" tag.
- **Overlays** · in-game toast (`overlay.html`) is a lower third: red REPLAY block + panel with a blue underline that wipes open behind a blue bar. The splash is the logo top-left, a big condensed "Rikette", a blinking red dot with the small status and a thin blue progress bar.
- Release notes · after an update the app opens the "What's new" dialog once. Notes live in `apps/desktop/src/renderer/src/lib/releaseNotes.ts`, keyed by the `package.json` version; add an entry (short title, plain-language bullets) before shipping. A version without an entry shows nothing.

## Website layout
- `topBar()` in `og.ts` · solid header with a hairline: logo + condensed wordmark (TLD muted) left; Account link and a small Download button right.
- **Landing** (`/`) · a full-width hero over a looping, muted gameplay video (`public/media/hero.mp4`, 720p30, no audio track, poster `hero.jpg`; paused under reduced motion) behind a dark scrim: REPLAY bug, the huge two-line headline ("That play? / Already saved.", second line blue), the lede, the Download button with a small caption and pressable `Alt` + `1` caps (the real Alt+1 works too). The **buffer strip** sits at the bottom of the hero: 60 cells ticking every second, a red now line; pressing the hotkey flashes the cells and wipes in a "Clip saved" lower third with the filename the app would write. A small "Rust · Built on OBS · Open source" line sits under the Download button, its "Open source" linking to the public desktop repo (`SOURCE_URL` in `@echoframe/shared`), which the top bar (hidden on phones) and the footer also link to as "GitHub". Then "How it works" (three steps), "Under the hood" (Fast, Light, Written in Rust, Open source, Built on OBS as numbered cells in one hairline grid with a blue top border; five across on wide screens), the **Export for Discord** spotlight (blue top border, a copy of the app's drag card, a size meter against the 20 MB target), "The rundown" (every app feature as numbered rows), pricing (Free and Pro cards), a "Why Rikette" call-out with a blue left bar and a small footer. Keep the copy in step with the app (e.g. `DISCORD_LIMIT_BYTES`, plans in `@echoframe/shared`).
- `.panel` (login, billing, not found) is a card with a blue top border that wipes in; `.wrap` (account, clip) is `--measure` wide. Account clip tiles get a blue border on hover. Plyr on the clip page uses the blue tokens.
- Page files (`landing.ts`, `og.ts`, `routes/admin.ts`) do layout only, with tokens. No hex outside `theme.ts`.

## Motion
Cuts and wipes, quick and linear-ish (`cubic-bezier(0.65, 0, 0.35, 1)`), never bouncy: pages wipe in from the left on tab switch, titles and lower thirds slide in behind a mask, lists rise a few pixels with a short stagger, the tab rule slides, the REC dot blinks on/off, the buffer strip ticks once a second, a clip save flashes the frame white. Keep transitions ≤ 400 ms. Everything is disabled under `prefers-reduced-motion`.
