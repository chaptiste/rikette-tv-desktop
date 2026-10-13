import { useEffect, useState } from 'react'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import type { CaptureStatus } from '../../../tauri/api'

export type RecMode = 'recording' | 'paused' | 'starting' | 'saving' | 'offline'

export function recMode(capture: CaptureStatus | null): RecMode {
  if (!capture) return 'offline'
  if (capture.paused) return 'paused'
  if (capture.starting) return 'starting'
  if (capture.saving && capture.ready) return 'saving'
  if (capture.ready) return 'recording'
  return 'offline'
}

/** Live capture status from the main process. */
export function useCaptureStatus(): [CaptureStatus | null, (next: CaptureStatus) => void] {
  const [capture, setCapture] = useState<CaptureStatus | null>(null)
  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getCaptureStatus().then(setCapture)
    return ipc.onCaptureStatus(setCapture)
  }, [])
  return [capture, setCapture]
}
