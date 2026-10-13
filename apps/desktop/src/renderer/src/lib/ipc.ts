export const ipc = window.echoframe

export function hasDesktopApi(): boolean {
  return typeof window.echoframe?.getSettings === 'function'
}

/** Commands reject with the message the app returned. */
export function ipcErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
