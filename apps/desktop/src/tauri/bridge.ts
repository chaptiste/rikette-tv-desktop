/**
 * `window.echoframe`: the app API the renderer uses (./api.ts), backed by the commands in
 * src-tauri/src/ipc.rs. Loaded before the renderer's own modules (see vite.config.ts).
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { EchoFrameApi } from './api'

declare global {
  interface Window {
    __RIKETTE__?: { platform: NodeJS.Platform }
  }
}

/** Subscribe now, return an unsubscribe that works even before `listen` resolves. */
function on<T>(event: string, listener: (payload: T) => void): () => void {
  let stopped = false
  let unlisten: (() => void) | null = null
  void listen<T>(event, (message: Event<T>) => listener(message.payload)).then((stop) => {
    if (stopped) stop()
    else unlisten = stop
  })
  return () => {
    stopped = true
    unlisten?.()
  }
}

const api: EchoFrameApi = {
  platform: window.__RIKETTE__?.platform ?? 'win32',
  getVersion: () => invoke('app_version'),
  getSettings: () => invoke('settings_get'),
  checkForUpdates: () => invoke('app_check_updates'),
  takeVersionChange: () => invoke('app_version_change'),
  updateSettings: (patch) => invoke('settings_update', { patch }),
  hotkeyRegistered: () => invoke('settings_hotkey_status'),
  listDisplays: () => invoke('settings_displays'),
  listAudioDevices: () => invoke('settings_audio_devices'),
  listWindows: () => invoke('settings_windows'),
  pickOutputFolder: () => invoke('settings_pick_output_folder'),
  getCaptureStatus: () => invoke('capture_status'),
  pauseBuffer: () => invoke('capture_pause'),
  resumeBuffer: () => invoke('capture_resume'),
  restartCapture: () => invoke('capture_restart'),
  saveClip: () => invoke('clips_save'),
  listClips: () => invoke('clips_list'),
  getClip: (id) => invoke('clips_get', { id }),
  deleteClip: (id) => invoke('clips_delete', { id }),
  deleteLocalFile: (id) => invoke('clips_delete_local', { id }),
  removeFromCloud: (id) => invoke('clips_remove_cloud', { id }),
  revealClip: (id) => invoke('clips_reveal', { id }),
  renameClip: (id, title) => invoke('clips_rename', { id, title }),
  clipMenu: (id) => invoke('clips_menu', { id }),
  listFolders: () => invoke('folders_list'),
  createFolder: (name) => invoke('folders_create', { name }),
  renameFolder: (id, name) => invoke('folders_rename', { id, name }),
  deleteFolder: (id) => invoke('folders_delete', { id }),
  moveClips: (ids, folderId) => invoke('clips_move', { ids, folderId }),
  getClipAudio: (id) => invoke('clips_audio', { id }),
  trimClip: (request) => invoke('clips_trim', { request }),
  exportForDiscord: (request) => invoke('clips_export_discord', { request }),
  startDiscordDrag: (id) => {
    void invoke('clips_discord_drag', { id })
  },
  revealDiscordExport: (id) => invoke('clips_discord_reveal', { id }),
  copyClipLink: (id) => invoke('clips_copy_link', { id }),
  uploadClip: (id) => invoke('clips_upload', { id }),
  getMe: () => invoke('auth_me'),
  refreshMe: () => invoke('auth_refresh'),
  signIn: () => invoke('auth_sign_in'),
  devLogin: (email) => invoke('auth_dev_login', { email }),
  signOut: () => invoke('auth_sign_out'),
  startCheckout: (interval) => invoke('billing_checkout', { interval }),
  openBillingPortal: () => invoke('billing_portal'),
  onCaptureStatus: (listener) => on('capture:status', listener),
  onClipsChanged: (listener) => on('clips:changed', () => listener()),
  onExportProgress: (listener) => on('clips:export-progress', listener),
  onDiscordReady: (listener) => on('clips:discord-ready', listener),
  onNavigate: (listener) => on('app:navigate', listener),
  onAuthChanged: (listener) => on('auth:changed', listener),
  onConfirm: (listener) => on('app:confirm', listener),
  onToast: (listener) => on('app:toast', listener),
  replyConfirm: (id, ok) => {
    void invoke('app_confirm_result', { reply: { id, ok } })
  },
  // Minimising and closing hide the window to the tray (src-tauri/src/window.rs).
  minimizeWindow: () => void getCurrentWindow().minimize(),
  toggleMaximizeWindow: () => void getCurrentWindow().toggleMaximize(),
  closeWindow: () => void getCurrentWindow().close()
}

window.echoframe = api

// The title bar is marked `.drag` (and controls `.no-drag`); dragging it moves the window.
const NO_DRAG =
  '.no-drag, a, button, input, label, textarea, [role="button"], [role="combobox"], [role="slider"], [data-slot="toggle-group"]'
window.addEventListener('mousedown', (event) => {
  if (event.button !== 0) return
  const target = event.target instanceof Element ? event.target : null
  if (!target?.closest('.drag') || target.closest(NO_DRAG)) return
  event.preventDefault()
  const win = getCurrentWindow()
  void (event.detail === 2 ? win.toggleMaximize() : win.startDragging())
})
