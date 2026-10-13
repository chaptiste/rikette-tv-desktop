import { customAlphabet } from 'nanoid'

const CLIP_ID_ALPHABET = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'
export const CLIP_ID_LENGTH = 8

const nanoidClip = customAlphabet(CLIP_ID_ALPHABET, CLIP_ID_LENGTH)

export function createClipId(): string {
  return nanoidClip()
}

export function clipPagePath(id: string): string {
  return `/c/${id}`
}
