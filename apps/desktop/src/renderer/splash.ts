import '@fontsource-variable/archivo/standard.css'
import '@fontsource-variable/martian-mono/standard.css'

function setStatus(label: string, percent: number | null): void {
  const status = document.querySelector('.status')
  const track = document.querySelector<HTMLElement>('.track')
  const fill = document.querySelector<HTMLElement>('.fill')
  if (status) status.textContent = label
  if (!track || !fill) return
  track.classList.toggle('busy', percent === null)
  fill.style.width = percent === null ? '' : `${Math.max(0, Math.min(100, percent))}%`
}

;(window as unknown as { setStatus: typeof setStatus }).setStatus = setStatus
