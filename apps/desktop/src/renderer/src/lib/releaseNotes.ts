/* Shown once after an update, in a modal. Add an entry, newest first, with the
 * same version as package.json before shipping a release. Versions without an
 * entry show nothing. */

export interface ReleaseNote {
  version: string
  date: string
  title: string
  items: string[]
}

export const RELEASE_NOTES: ReleaseNote[] = [
  {
    version: '0.2.7',
    date: '2026-10-26',
    title: 'A new look',
    items: [
      'Rikette has a new look: Tokyo Night colours, a monospace interface with clean titles, square corners and a new pixel logo.',
      'The Replay screen now shows the buffer status at the top (time held, game, video and encoder) with Clip it and Pause right there, and your eight most recent clips below.',
      'New clips get a NEW tag and a blue outline for their first two minutes.'
    ]
  },
  {
    version: '0.2.6',
    date: '2026-10-24',
    title: 'Your replays at a glance',
    items: [
      'The new Dashboard brings your latest replay, three earlier clips and capture settings into one screen, with no scrolling at the minimum window size.',
      'Save a clip, pause or resume recording, restart the recorder and change your hotkey directly from the dashboard.',
      'See your capture source, replay length, video output, encoder, microphone and local library usage. The animated buffer timeline has been removed.'
    ]
  },
  {
    version: '0.2.5',
    date: '2026-10-16',
    title: 'Heads-up when Pro ends',
    items: [
      'If your Pro plan ends while you have more cloud clips than the free plan keeps, a banner under the top bar tells you how many will be deleted from the cloud and when, with a button to renew. Clips on your PC are never touched.'
    ]
  },
  {
    version: '0.2.4',
    date: '2026-10-14',
    title: 'Smaller Discord drag preview',
    items: [
      'Dragging a clip into Discord shows a small preview under the cursor instead of a huge see-through picture.'
    ]
  },
  {
    version: '0.2.3',
    date: '2026-10-14',
    title: 'Easier to read',
    items: [
      'Small text is bigger across the app: section labels everywhere, and on Replay the clip details, durations, buffer timeline and hotkey hint.'
    ]
  },
  {
    version: '0.2.1',
    date: '2026-10-13',
    title: 'Drag clips into folders again',
    items: [
      'You can drag clips onto a folder in Clips again, as well as onto All clips or New folder. This stopped working in 0.2.0.'
    ]
  },
  {
    version: '0.2.0',
    date: '2026-10-13',
    title: 'Lighter, faster, open source',
    items: [
      'Rikette has been rebuilt in Rust on top of OBS. The download is a fraction of the size, and the app uses much less memory, especially while it sits in the tray.',
      'Recording runs in its own process: if a game or OBS crashes, Rikette keeps going and restarts the recorder by itself.',
      'The desktop app is now open source (GPL-3.0). The code is on GitHub at chaptiste/rikette-tv-desktop.',
      'Your clips, folders, settings and sign-in carry over from the previous version.'
    ]
  },
  {
    version: '0.1.33',
    date: '2026-10-12',
    title: 'Smoother start and Clips',
    items: [
      'Rikette no longer freezes while the replay buffer starts. Recording now runs in its own background process, so the window stays responsive the whole time.',
      'The Clips page opens faster: thumbnails are smaller, load as you scroll and are kept between visits.',
      'Clips you add to your clips folder by hand still show up, now checked in the background instead of each time you open Clips.'
    ]
  },
  {
    version: '0.1.32',
    date: '2026-10-12',
    title: 'A new logo',
    items: [
      'Rikette has a new logo to match the replay look: a bold R, the red recording dot, and a small strip of blue seconds ending at a red "now" line.',
      'You will see it on the app icon, the taskbar, the tray, the start-up screen and the website.'
    ]
  },
  {
    version: '0.1.31',
    date: '2026-10-12',
    title: 'Folders for your clips',
    items: [
      'Sort your clips into folders. The new list on the left of Clips shows All clips, Unsorted and your folders, each with a clip count.',
      'Drag a clip onto a folder to file it, or use "Move to folder" in the clip\'s ⋯ menu or right-click menu. Every move can be undone from the popup.',
      'Drop a clip on "New folder" to create a folder with that clip already inside.',
      'Double-click a folder to rename it. Deleting a folder never deletes clips: they go back to Unsorted.',
      'Folders only sort your library in Rikette. Your video files stay where they are on disk.'
    ]
  },
  {
    version: '0.1.30',
    date: '2026-10-11',
    title: 'Instant replay',
    items: [
      'A new look inspired by TV replays: deep navy, one bright blue, the red dot from the logo, sharp edges and a narrow, bold font.',
      'Home is now called Replay. It shows your last clip like a TV replay, and a strip of the seconds held in the buffer, filling up after a start and ending at a red "now" line.',
      'Your latest clips are listed on the right, newest first. Clips saved in the last two minutes are marked in blue.',
      'In the editor the playhead is now a red line, and times use a fixed-width font so the numbers stay still.',
      'The in-game "Clip saved" popup is now a TV-style banner with a red REPLAY tag.',
      'Installing an update from the version number in the bottom left now happens in the background: Rikette closes for a few seconds and reopens on its own, without the installer window.'
    ]
  },
  {
    version: '0.1.29',
    date: '2026-10-11',
    title: 'Brand new look',
    items: [
      'Rikette has a whole new look: a soft midnight blue theme, a new font, rounded buttons and smooth animations.',
      'The sidebar is gone: switch pages with the tabs at the top, or with Ctrl+1 to Ctrl+5.',
      'A status pill in the top bar always shows whether the replay buffer is recording, paused or offline.',
      'Home shows your capture setup, a big Save clip button and your last four clips. Saving a clip while it is open pops a "Clip saved" card.',
      'The Library is now called Clips and shows your clips as a grid of big thumbnails.',
      'Settings are split into tabs: Capture, Audio, Video, Controls and System. Account is now called Profile.',
      'The in-game "Clip saved" popup got a fresh look too.'
    ]
  },
  {
    version: '0.1.28',
    date: '2026-10-11',
    title: 'Export for Discord',
    items: [
      'New "Export for Discord" button in the editor and in each clip\'s menu in the Library. It makes a copy under 20 MB so you can post it on Discord without Nitro.',
      'Rikette picks the best resolution and frame rate that fit: short clips stay 1080p 60 fps, longer ones get a little smaller.',
      'In the editor, the export keeps your trim and audio volumes, and mixes game and mic audio into one track so Discord plays both.',
      'When the export is done, a card pops up at the bottom of the window: drag it straight into a Discord chat. Your original clip is not changed.',
      'Exports are also kept in the "Discord exports" folder inside your clips folder.'
    ]
  },
  {
    version: '0.1.27',
    date: '2026-10-10',
    title: 'Simpler account page',
    items: [
      'The API address field is gone from the Account page. Rikette always connects to rikette.tv, so there is nothing to set up.'
    ]
  },
  {
    version: '0.1.26',
    date: '2026-10-10',
    title: 'Updates on launch',
    items: [
      'When you open Rikette, it first checks for an update. If there is one, it downloads and installs it, then reopens on its own.',
      'If you are offline or the update server is slow, Rikette opens right away and keeps updating in the background.',
      'When Rikette starts with Windows, it still opens quietly in the tray without the update screen.'
    ]
  },
  {
    version: '0.1.25',
    date: '2026-10-10',
    title: 'Steadier recording',
    items: [
      'If the capture process crashes, Rikette now restarts it on its own instead of waiting for you.',
      'After you resume the buffer, Rikette only shows Recording once the buffer is actually back.',
      'Lots of behind-the-scenes cleanup to make capture easier to fix and improve.'
    ]
  },
  {
    version: '0.1.24',
    date: '2026-10-06',
    title: 'No more black clips',
    items: [
      'When Rikette cannot capture a game directly (some anti-cheats block it), it records your screen instead of saving a black video.',
      'Rikette now keeps a log file, so we can look into problems you report.'
    ]
  },
  {
    version: '0.1.23',
    date: '2026-09-28',
    title: 'A new look',
    items: [
      'Rikette now matches rikette.tv: beige keycaps on a dark desk, and no more blue.',
      'Buttons are keys you press. Main actions are beige, everything else is a dark key.',
      'Your save-clip hotkey shows up as real keycaps on Home, in the Library and in Settings.',
      'Switches, sliders, the storage bar, the trim selection and the audio waveforms use the new colours.',
      'The in-game "Clip Saved" popup has the new look too.'
    ]
  }
]

function parts(version: string): number[] {
  return version.split(/[.-]/).map((part) => Number.parseInt(part, 10) || 0)
}

export function compareVersions(a: string, b: string): number {
  const x = parts(a)
  const y = parts(b)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const diff = (x[i] ?? 0) - (y[i] ?? 0)
    if (diff !== 0) return diff
  }
  return 0
}

/** Notes for every version after `from` up to `to`, newest first. Unknown `from` shows `to` only. */
export function notesBetween(from: string | null, to: string): ReleaseNote[] {
  return RELEASE_NOTES.filter((note) =>
    from === null
      ? compareVersions(note.version, to) === 0
      : compareVersions(note.version, from) > 0 && compareVersions(note.version, to) <= 0
  ).sort((a, b) => compareVersions(b.version, a.version))
}
