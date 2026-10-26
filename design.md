# Design — Rikette.tv

Locked design system. Future runs read this file first; pages defer to it.
Amend intentionally — the file is the rule.

One theme on both surfaces: **Instant replay, terminal cut**. Rikette is a replay buffer, so the structure comes from TV sports replay graphics (REPLAY bugs, lower thirds, timecodes); the finish comes from a tiling-WM desktop in the style of omarchy.org: Tokyo Night colours, a monospace UI with clean sans titles, zero radius, flat hairline panels and bright blocks with dark ink. The signature element is the **pixel**: square cells (the logo's buffer strip, the website's wordmark and pixel field, the REC light), always square and flat.
- **Website** (`apps/web`, every page and email). Source of truth: `apps/web/src/theme.ts`.
- **Desktop app** (`apps/desktop`). Source of truth: `:root` in `apps/desktop/src/renderer/src/styles/globals.css` (copies in `overlay.html` and `splash.html`, which have no Tailwind). Keep its hex values in step with `PALETTE`.

## Never
Rounded corners of any kind, blurred colour blobs, gradients (fills, text or backgrounds; the only gradient allowed is a scrim behind text laid over footage), glows, glassmorphism/backdrop blur, pill-shaped buttons, rounded cards, bouncy easing, icon-in-a-tinted-circle feature cards, stat-card rows. If it looks like a generic SaaS template, it is wrong.

## Palette
Tokyo Night.
- Background `#1a1b26` · panel/card `#1f2230` · popover/raised `#24283b` · input `#16161e` · secondary fill `#24283b` · accent/hover `#292e42` · border `#2a2e42` · border-strong `#414868` · text `#c0caf5` · muted `#8b93b8`.
- Replay blue `#7aa2f7` (hover `#9ab8ff`, deep `#5a7fd6`, **dark ink `#16161e`**): the one accent. Filled buttons, the active tab rule, selection, the buffer cells, "new" markers. Anything filled with a bright colour (blue, red, green) carries dark ink, never white.
- Cyan `#7dcfff`: links, focus rings and the ten-second markers in the strip only.
- Red `#f7768e` (the logo pixel): REC light, REPLAY tags, the "now" line and the editor playhead. Never for buttons except destructive confirms (`#ff8fa3`).
- Status: ok `#9ece6a`, warn/paused `#e0af68`.
- Text over footage sits on `--scrim` (`rgb(22 22 30 / 85%)`).

## Type
- **JetBrains Mono** for the UI: body text, button labels, inputs, timecodes, sizes, units, tab numbers and small uppercase labels (`eyebrow`, `tc`). Uppercase is only for those small labels and tags.
- **Geist** for titles and names: `display-title` is 600 with tight tracking (−0.035em) and leading ~1.05; `font-display` (clip titles, tabs, big numbers) is Geist at the weight the component sets. Never heavy or condensed.
- The app bundles both via `@fontsource-variable/jetbrains-mono` and `@fontsource-variable/geist`; the website loads them from Google Fonts in `page()`; emails use the system mono stack.

## Logo
A square Tokyo Night tile (no rounded corners) with the pixel R from the website's wordmark (`wordmark()` in `apps/web/src/landing.ts`: thick two-cell verticals, one-row bars, rows banded light to deep blue) and the red 2×2 REC pixel top-right, exactly as it ends the wordmark. The cells sit on a 48-unit grid with 16-unit offsets so the 32 px icon stays crisp. The source is `apps/desktop/build/logo.svg`. `node scripts/build-logo.mjs` renders every PNG/ICO/ICNS from it (app icon, tray, splash, in-app logo, website favicon and email logo), so edit the SVG and never the PNGs.

## Components
- **Buttons** · flat blocks, no radius, mono medium label. Default = solid blue with dark ink; `secondary`/`outline` = transparent with a 1 px `border-strong` outline; `.btn-danger` = red. Hover lightens, press drops 1 px. No shadows. App: `Button` (`.btn`, `.btn-secondary`, `.btn-danger`); web `.btn`, `.btn-dark`, `.btn-sm`, `.btn-lg` (a `<small>` inside a button is a small caption).
- **Cards** · flat `--card` panels with a 1 px border and no radius. Emphasis is a 2 px blue top border (dialogs, featured tier, sign-in panels) or a 3 px blue left bar (selected rows, call-outs) — never a shadow or glow.
- **Bug** (`.bug`) · the TV replay tag: a red block ("REPLAY", mono bold, dark ink) next to a dark small timecode block. `.bug-blue` swaps the red for blue.
- **Lower third** · a blue block (number or duration) next to a raised panel with a small mono kicker and a Geist title, a 2 px blue underline; it wipes in from the left. Used for "Clip saved" in the app, the in-game overlay and the landing page.
- **Key caps** · small uppercase mono caps with a 2 px bottom border (`Kbd` → `.kbd-key`; web `.kbd`). On a blue button use `.kbd-on-primary`.
- **Form controls** · shadcn primitives: square-cornered inputs/selects with a blue focus border, rectangular switches with a square knob (dark when on), a thin slider track with a bar thumb, a flat blue progress bar, segmented toggle groups, small small badges.
- Emails: card with a blue top border, square buttons with a darker 2 px bottom edge, system mono text.

## Desktop app layout
- **Top bar** (`TopNav.tsx`) · logo + "Rikette" in a cell with a right border, then text tabs (Replay, Clips, Editor, Settings, Profile), each followed by its small Ctrl number; a 2 px blue rule slides under the active tab. The REC light (`RecChip.tsx`: REC / SAVING / STARTING / PAUSED / OFFLINE, blinking square red pixel, buffer seconds) sits on the right. The Windows title-bar overlay is 44 px tall and `#1a1b26` (`window.rs` and `splash.rs` set it).
- **PageHeader** · small kicker + a 40 px Geist title, actions on the right. Without a title it is a toolbar row (the editor).
- **Replay** (`/`) · one column. On top, the **buffer status** card (2 px blue top border): a small "Replay buffer" kicker with the blinking REC pixel, the state title ("Rolling." / "Paused." / "Warming up." / "Cutting…" / "Off air.") and a muted mono subline; on the right Pause/Resume (Restart when offline), the `xl` "Clip it" button with the hotkey caps and a small "change hotkey" link; along the bottom a hairline row of four mono readouts (in buffer `held / total`, game or source, video, encoder). Below, **Recent clips**: the newest eight as a four-column thumbnail grid (duration chip, title, game · age); clips under two minutes old get a blue outline and a `NEW` bug; the newest card flashes white while saving and shows the "Clip saved" lower third when a clip lands. No buffer timeline.
- **Clips** (`/library`) · thumbnail grid with square corners; status badge top-left, game name and small duration over a bottom scrim; hover = 2 px blue outline and a blue play square; upload/link and menu chips on hover.
- **Editor** · square video with a hairline outline, small meta chips; timeline in a card with a blue selection and square handles; the playhead is red. Audio lanes: desktop light blue, mic `--ok`, other muted; their playhead is red too. Times use tabular figures.
- **Settings** · numbered rundown list on the left (01 Capture … 05 System, active row = card fill + blue left bar); the panel shows the section title, "01 / 05" and rows separated by hairlines.
- **Profile** (`/account`) · square blue initial block, small plan tag, storage bar; tier cards with a 48 px Geist price, the featured one with a blue top border and a green "SAVE x%" tag.
- **Overlays** · in-game toast (`overlay.html`) is a lower third: red REPLAY block + panel with a blue underline that wipes open behind a blue bar. The splash is the logo top-left, a big Geist "Rikette", a blinking square red pixel with the small status and a thin blue progress bar.
- Release notes · after an update the app opens the "What's new" dialog once. Notes live in `apps/desktop/src/renderer/src/lib/releaseNotes.ts`, keyed by the `package.json` version; add an entry (short title, plain-language bullets) before shipping. A version without an entry shows nothing.

## Website layout
- `topBar()` in `og.ts` · solid header with a hairline: logo + Geist wordmark (TLD muted) left; Account link and a small Download button right.
- **Landing** (`/`) · the hero, in the omarchy.org mould: a pixel **wordmark** (`wordmark()` in `landing.ts`, a 7-row bitmap font; rows banded light to deep blue, a blinking red REC pixel after the last letter) over a seeded **pixel field** of buffer cells, dense at the bottom and the far sides and clear behind the text; then the mono h1 "Free, open-source & light game clipping for Windows", the lede, Download for Windows + Read the code, and the three **pillars** in one hairline grid with a blue top border: €0 Free (→ pricing), GPL-3.0 Open source (→ under the hood), 6.9 MB Light (→ Task Manager). Then "See it in action" (the looping gameplay clip from `public/media/`, framed with a REPLAY bug, plays only on screen and never under reduced motion; pressable `Alt` + `1` caps, and the real Alt+1, flash the frame and wipe in a "Clip saved" lower third with the filename the app would write; the three steps beside it), "Next to nothing" (the Task Manager proof of light), "Open source, under the hood" (Open source, Written in Rust, Built on OBS as numbered cells; a Read the code button links `SOURCE_URL` in `@echoframe/shared`, which the top bar, hidden on phones, and the footer also link to as "GitHub"), the **Export for Discord** spotlight (blue top border, a copy of the app's drag card, a size meter against the 20 MB target), "The rundown" (every app feature as numbered rows), pricing (Free is the featured card with the blue top border and the primary button; Pro is secondary), a "Why Rikette" call-out with a blue left bar and a small footer. Keep the copy in step with the app (e.g. `DISCORD_LIMIT_BYTES`, plans in `@echoframe/shared`).
- `.panel` (login, billing, not found) is a card with a blue top border that wipes in; `.wrap` (account, clip) is `--measure` wide. Account clip tiles get a blue border on hover. Plyr on the clip page uses the blue tokens.
- Page files (`landing.ts`, `og.ts`, `routes/admin.ts`) do layout only, with tokens. No hex outside `theme.ts`.

## Motion
Cuts and wipes, quick and linear-ish (`cubic-bezier(0.65, 0, 0.35, 1)`), never bouncy: pages wipe in from the left on tab switch, titles and lower thirds slide in behind a mask, lists rise a few pixels with a short stagger, the tab rule slides, the REC pixel blinks on/off, a few cells of the website's pixel field blink, a clip save flashes the frame white. Keep transitions ≤ 400 ms. Everything is disabled under `prefers-reduced-motion`.
