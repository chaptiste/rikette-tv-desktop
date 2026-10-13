import type { EchoFrameApi } from './api'

declare global {
  interface Window {
    echoframe: EchoFrameApi
  }
}

export {}
