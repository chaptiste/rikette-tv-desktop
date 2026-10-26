import '@fontsource-variable/jetbrains-mono/wght.css'
import '@fontsource-variable/geist/wght.css'

let hideTimer: ReturnType<typeof setTimeout> | null = null
let audio: HTMLAudioElement | null = null

function playSfx(): Promise<void> {
  const src = (window as unknown as { __sfx?: string }).__sfx
  if (!src) return Promise.resolve()
  if (!audio) {
    audio = new Audio(src)
    audio.preload = 'auto'
  }
  audio.pause()
  audio.currentTime = 0
  audio.volume = 1
  return audio.play().then(() => undefined)
}

function showToast(title: string): Promise<void> {
  const el = document.getElementById('toast')
  const label = el?.querySelector('.label')
  if (el && label) {
    label.textContent = title
    el.classList.add('show')
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = setTimeout(() => {
      el.classList.remove('show')
    }, 2800)
  }
  return playSfx()
}

;(window as unknown as { showToast: (title: string) => Promise<void> }).showToast = showToast
