export const encoderIds = [
  'jim_nvenc',
  'ffmpeg_nvenc',
  'h264_amf',
  'obs_qsv11',
  'vt_h264',
  'x264'
] as const

export type EncoderId = (typeof encoderIds)[number]

export const windowsEncoderPreference = [
  'jim_nvenc',
  'ffmpeg_nvenc',
  'h264_amf',
  'obs_qsv11',
  'x264'
] as const satisfies readonly EncoderId[]

export const macEncoderPreference = ['vt_h264', 'x264'] as const satisfies readonly EncoderId[]

export const encoderLabels: Record<EncoderId, string> = {
  jim_nvenc: 'NVIDIA NVENC',
  ffmpeg_nvenc: 'NVIDIA NVENC (FFmpeg)',
  h264_amf: 'AMD AMF',
  obs_qsv11: 'Intel Quick Sync',
  vt_h264: 'VideoToolbox',
  x264: 'x264 (CPU)'
}

export const CLOUD_VIDEO_CODEC = 'h264'
export const CLOUD_AUDIO_CODEC = 'aac'
export const CLOUD_CONTAINER = 'mp4'
