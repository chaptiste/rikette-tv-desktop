export const APP_NAME = 'Rikette'
export const SITE_NAME = 'Rikette.tv'
export const APP_ID = 'tv.rikette.app'
export const APP_PROTOCOL = 'rikette'
export const APP_ORIGIN = 'https://rikette.tv'
export const CLIP_FOLDER_NAME = 'Rikette'
export const RELEASES_REPO = 'chaptiste/rikette-tv-desktop'
export const RELEASES_LATEST = `https://github.com/${RELEASES_REPO}/releases/latest`
/** The desktop app's public source, which also hosts its releases. */
export const SOURCE_URL = `https://github.com/${RELEASES_REPO}`

export function protocolUrl(pathAndQuery: string): string {
  const suffix = pathAndQuery.startsWith('/') ? pathAndQuery.slice(1) : pathAndQuery
  return `${APP_PROTOCOL}://${suffix}`
}

export function isAppProtocolUrl(url: string): boolean {
  return url.startsWith(`${APP_PROTOCOL}://`) || url.startsWith('echoframe://')
}
