export const FREE_CLOUD_CLIPS = 3
export const PRO_BYTES = 150 * 1024 ** 3
// After Pro ends, clips over the free limit are kept this long, then deleted.
export const CLEANUP_GRACE_DAYS = 30
export const PRO_PRICE_MONTHLY_USD = 4.99
export const PRO_PRICE_YEARLY_USD = 39
export const MAX_UPLOAD_BYTES = 500 * 1024 * 1024
export const CLOUD_BITRATE_CEILING_BPS = 12_000_000

export function estimatedUploadBytes(durationMs: number): number {
  const seconds = Math.max(1, durationMs / 1000)
  return Math.min(MAX_UPLOAD_BYTES, Math.ceil((CLOUD_BITRATE_CEILING_BPS / 8) * seconds * 1.2))
}
