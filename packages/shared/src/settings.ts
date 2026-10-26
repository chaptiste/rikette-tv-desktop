import { z } from 'zod'
import { captureModeSchema } from './api'
import { encoderIds } from './codecs'

export const BUFFER_SECONDS = [15, 30, 60] as const
export type BufferSeconds = (typeof BUFFER_SECONDS)[number]

export const qualityPresets = ['high', 'medium', 'low'] as const
export type QualityPreset = (typeof qualityPresets)[number]

export const qualityCqp = {
  high: 15,
  medium: 18,
  low: 23
} as const satisfies Record<QualityPreset, number>

export const qualityUploadCaps = {
  high: { height: 1440, fps: 60 },
  medium: { height: 1080, fps: 60 },
  low: { height: 1080, fps: 30 }
} as const satisfies Record<QualityPreset, { height: number; fps: number }>

export const OUTPUT_RES = ['native', '1440', '1080', '720'] as const
export type OutputRes = (typeof OUTPUT_RES)[number]

export const NVENC_PRESETS = ['p1', 'p4', 'p7'] as const
export type NvencPreset = (typeof NVENC_PRESETS)[number]

export const encoderPrefSchema = z.union([z.literal('auto'), z.enum(encoderIds)])
export type EncoderPref = z.infer<typeof encoderPrefSchema>

export const bufferSecondsSchema = z.union([
  z.literal(15),
  z.literal(30),
  z.literal(60)
])

export const fpsSchema = z.union([z.literal(30), z.literal(60)])
export type Fps = z.infer<typeof fpsSchema>

export const appSettingsSchema = z.object({
  bufferSec: bufferSecondsSchema,
  captureMode: captureModeSchema,
  monitorIndex: z.number().int().nonnegative(),
  /** Window mode's window as OBS's `title:class:exe` id; empty until picked. */
  captureWindow: z.string(),
  micEnabled: z.boolean(),
  desktopAudioId: z.string().min(1),
  micDeviceId: z.string().min(1),
  micVolume: z.number().int().min(0).max(200),
  hotkey: z.string().min(1),
  quality: z.enum(qualityPresets),
  outputRes: z.enum(OUTPUT_RES),
  encoderPref: encoderPrefSchema,
  nvencPreset: z.enum(NVENC_PRESETS),
  nvencMultipass: z.boolean(),
  fps: fpsSchema,
  outputFolder: z.string().min(1),
  autostart: z.boolean(),
  fallbackToDisplay: z.boolean(),
  encoderId: z.string(),
  encoderLabel: z.string(),
  apiBase: z.string().url()
})

export type AppSettings = z.infer<typeof appSettingsSchema>

export const DEFAULT_HOTKEY = 'Alt+1'

export function defaultAppSettings(
  outputFolder: string,
  apiBase = 'https://rikette.tv'
): AppSettings {
  return {
    bufferSec: 60,
    captureMode: 'auto-game',
    monitorIndex: 0,
    captureWindow: '',
    micEnabled: false,
    desktopAudioId: 'default',
    micDeviceId: 'default',
    micVolume: 100,
    hotkey: DEFAULT_HOTKEY,
    quality: 'high',
    outputRes: 'native',
    encoderPref: 'auto',
    nvencPreset: 'p4',
    nvencMultipass: false,
    fps: 60,
    outputFolder,
    autostart: false,
    fallbackToDisplay: true,
    encoderId: '',
    encoderLabel: '',
    apiBase
  }
}

export function formatHotkey(accelerator: string, platform: string): string {
  let formatted = accelerator
  if (platform === 'darwin') {
    formatted = formatted
      .replaceAll('CommandOrControl', '⌘')
      .replaceAll('Command', '⌘')
      .replaceAll('Super', '⌘')
      .replaceAll('Control', '⌃')
      .replaceAll('Alt', 'Option')
  } else {
    formatted = formatted
      .replaceAll('CommandOrControl', 'Ctrl')
      .replaceAll('Command', 'Win')
      .replaceAll('Super', 'Win')
  }
  return formatted.replaceAll('+', ' + ')
}
