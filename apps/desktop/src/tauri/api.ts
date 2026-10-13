import type { AppSettings, BillingInterval, CloudStatus, Me } from '@echoframe/shared'

export interface AudioDeviceInfo {
  id: string
  label: string
}

export interface AudioDevices {
  outputs: AudioDeviceInfo[]
  inputs: AudioDeviceInfo[]
}

export interface DisplayInfo {
  id: number
  index: number
  label: string
  bounds: { x: number; y: number; width: number; height: number }
  primary: boolean
}

export interface SettingsUpdateResult {
  settings: AppSettings
  hotkeyRegistered: boolean
  capture?: CaptureStatus
}

export interface CaptureTarget {
  kind: 'game' | 'display'
  exe: string
  title: string
}

export interface EncoderInfo {
  id: string
  label: string
}

export interface CaptureStatus {
  ready: boolean
  paused: boolean
  saving: boolean
  starting: boolean
  encoderId: string
  encoderLabel: string
  availableEncoders: EncoderInfo[]
  error: string | null
  bufferSec: number
  /** When the replay buffer last started filling (epoch ms), or null when it is not buffering. */
  bufferedSince: number | null
  captureTarget: CaptureTarget
  outputWidth: number
  outputHeight: number
  fps: number
}

export interface LibraryClip {
  id: string
  title: string
  createdAt: number
  durationMs: number
  width: number
  height: number
  bytes: number
  gameTitle: string | null
  cloudStatus: CloudStatus
  cloudId: string | null
  publicUrl: string | null
  localFile: boolean
  videoUrl: string
  thumbUrl: string | null
  /** null = Unsorted. */
  folderId: string | null
}

/** A library-only grouping: filing a clip never moves its file on disk. */
export interface ClipFolder {
  id: string
  name: string
  createdAt: number
}

/** Omitted fields export the whole clip at its original volumes. */
export interface DiscordExportRequest {
  id: string
  startSec?: number
  endSec?: number
  volumes?: number[]
}

/** `percent` is null once the export has finished or failed. */
export interface DiscordExportProgress {
  id: string
  title: string
  percent: number | null
}

/** A finished export, ready to be dragged out of the app. */
export interface DiscordExportReady {
  id: string
  title: string
  bytes: number
  thumbUrl: string | null
}

export interface TrimClipRequest {
  id: string
  startSec: number
  endSec: number
  mode: 'overwrite' | 'save-as-new'
  volumes?: number[]
}

export type ClipAudioKind = 'desktop' | 'mic' | 'audio'

export interface ClipAudioTrack {
  id: ClipAudioKind
  label: string
  index: number
  peaks: number[]
  durationSec: number
  previewUrl: string
}

/** Set on the first launch after an update. `from` is null when the old build predates tracking. */
export interface VersionChange {
  from: string | null
  to: string
}

export interface AppConfirm {
  id: string
  title: string
  description: string
  confirmLabel?: string
  cancelLabel?: string
  destructive?: boolean
}

export interface AppToast {
  title: string
  description?: string
  kind?: 'info' | 'success' | 'warning' | 'error'
}

export interface EchoFrameApi {
  platform: NodeJS.Platform
  getVersion: () => Promise<string>
  getSettings: () => Promise<AppSettings>
  checkForUpdates: () => Promise<void>
  takeVersionChange: () => Promise<VersionChange | null>
  updateSettings: (patch: Partial<AppSettings>) => Promise<SettingsUpdateResult>
  hotkeyRegistered: () => Promise<boolean>
  listDisplays: () => Promise<DisplayInfo[]>
  listAudioDevices: () => Promise<AudioDevices>
  pickOutputFolder: () => Promise<AppSettings>
  getCaptureStatus: () => Promise<CaptureStatus>
  pauseBuffer: () => Promise<CaptureStatus>
  resumeBuffer: () => Promise<CaptureStatus>
  restartCapture: () => Promise<CaptureStatus>
  saveClip: () => Promise<void>
  listClips: () => Promise<LibraryClip[]>
  getClip: (id: string) => Promise<LibraryClip | null>
  deleteClip: (id: string) => Promise<boolean>
  deleteLocalFile: (id: string) => Promise<boolean>
  removeFromCloud: (id: string) => Promise<boolean>
  revealClip: (id: string) => Promise<void>
  renameClip: (id: string, title: string) => Promise<LibraryClip>
  clipMenu: (id: string) => Promise<void>
  listFolders: () => Promise<ClipFolder[]>
  createFolder: (name: string) => Promise<ClipFolder>
  renameFolder: (id: string, name: string) => Promise<ClipFolder>
  /** Asks first; resolves false when cancelled. */
  deleteFolder: (id: string) => Promise<boolean>
  /** `folderId` null moves the clips back to Unsorted. */
  moveClips: (ids: string[], folderId: string | null) => Promise<void>
  getClipAudio: (id: string) => Promise<ClipAudioTrack[]>
  trimClip: (request: TrimClipRequest) => Promise<LibraryClip>
  exportForDiscord: (request: DiscordExportRequest) => Promise<{ path: string; bytes: number }>
  startDiscordDrag: (id: string) => void
  revealDiscordExport: (id: string) => Promise<void>
  copyClipLink: (id: string) => Promise<string>
  uploadClip: (id: string) => Promise<string | null>
  getMe: () => Promise<Me | null>
  refreshMe: () => Promise<Me | null>
  signIn: () => Promise<void>
  devLogin: (email?: string) => Promise<Me>
  signOut: () => Promise<void>
  startCheckout: (interval: BillingInterval) => Promise<void>
  openBillingPortal: () => Promise<void>
  onCaptureStatus: (listener: (status: CaptureStatus) => void) => () => void
  onClipsChanged: (listener: () => void) => () => void
  onExportProgress: (listener: (progress: DiscordExportProgress) => void) => () => void
  onDiscordReady: (listener: (ready: DiscordExportReady) => void) => () => void
  onNavigate: (listener: (path: string) => void) => () => void
  onAuthChanged: (listener: (me: Me | null) => void) => () => void
  onConfirm: (listener: (ask: AppConfirm) => void) => () => void
  onToast: (listener: (toast: AppToast) => void) => () => void
  replyConfirm: (id: string, ok: boolean) => void
  /** The title bar's own buttons (Windows and Linux draw no native ones). */
  minimizeWindow: () => void
  toggleMaximizeWindow: () => void
  closeWindow: () => void
}
