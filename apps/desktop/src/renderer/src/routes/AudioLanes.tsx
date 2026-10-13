import { Fragment, useEffect, useRef } from 'react'
import { Volume2, VolumeX } from 'lucide-react'
import { Slider } from '@/components/ui/slider'
import { cn } from '@/lib/utils'
import type { ClipAudioTrack } from '../../../tauri/api'

/* Canvas needs literal colours, so lanes read the theme tokens from globals.css. */
const TRACK_TOKEN: Record<ClipAudioTrack['id'], string> = {
  desktop: '--highlight',
  mic: '--ok',
  audio: '--muted-foreground'
}

function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

export function AudioLanes(props: {
  tracks: ClipAudioTrack[]
  volumes: number[]
  duration: number
  start: number
  end: number
  current: number
  playing: boolean
  seekNonce: number
  mediaHeld: boolean
  disabled?: boolean
  onVolume: (index: number, percent: number) => void
  onSeek: (time: number) => void
}): React.JSX.Element | null {
  const mixer = useRef<TrackMixer | null>(null)
  // Latest values for the start effect, which must not re-run on every playhead tick.
  const playheadRef = useRef(props.current)
  const volumesRef = useRef(props.volumes)
  useEffect(() => {
    playheadRef.current = props.current
    volumesRef.current = props.volumes
  })
  const trackKey = props.tracks.map((track) => track.previewUrl).join('|')

  useEffect(() => {
    const next = new TrackMixer()
    mixer.current = next
    return () => {
      next.dispose()
      if (mixer.current === next) mixer.current = null
    }
  }, [trackKey])

  useEffect(() => {
    const engine = mixer.current
    if (!engine) return
    for (let i = 0; i < props.tracks.length; i++) {
      engine.setGain(i, (props.volumes[i] ?? 100) / 100)
    }
  }, [props.tracks, props.volumes])

  useEffect(() => {
    const engine = mixer.current
    if (!engine || !props.mediaHeld) return
    if (!props.playing) {
      engine.stop()
      return
    }
    void engine
      .start(props.tracks, volumesRef.current, playheadRef.current)
      .catch((error: unknown) => {
        console.warn('[echoframe] audio mix failed', error)
      })
  }, [props.playing, props.seekNonce, props.mediaHeld, props.tracks, trackKey])

  if (props.tracks.length === 0) return null

  return (
    <>
      {props.tracks.map((track, index) => {
        const volume = props.volumes[index] ?? 100
        return (
          <Fragment key={track.previewUrl}>
            <div className="min-w-0">
              <div className="eyebrow truncate text-foreground">{track.label}</div>
              <div className="text-[10px] tabular-nums text-muted-foreground">
                {Math.round(volume)}%
              </div>
            </div>
            <Waveform
              peaks={track.peaks}
              duration={props.duration}
              start={props.start}
              end={props.end}
              current={props.current}
              volume={volume}
              color={token(TRACK_TOKEN[track.id])}
              disabled={props.disabled}
              onSeek={props.onSeek}
            />
            <div className="flex items-center gap-1">
              <button
                type="button"
                className="flex size-6 shrink-0 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
                title={volume === 0 ? 'Unmute' : 'Mute'}
                disabled={props.disabled}
                onClick={() => props.onVolume(index, volume === 0 ? 100 : 0)}
              >
                {volume === 0 ? (
                  <VolumeX className="size-3.5" strokeWidth={1.5} />
                ) : (
                  <Volume2 className="size-3.5" strokeWidth={1.5} />
                )}
              </button>
              <Slider
                min={0}
                max={200}
                step={1}
                value={[volume]}
                disabled={props.disabled}
                onValueChange={([next]) => {
                  if (next === undefined) return
                  props.onVolume(index, next)
                }}
              />
            </div>
          </Fragment>
        )
      })}
    </>
  )
}

function Waveform(props: {
  peaks: number[]
  duration: number
  start: number
  end: number
  current: number
  volume: number
  color: string
  disabled?: boolean
  onSeek: (time: number) => void
}): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const duration = Math.max(props.duration, 0.001)
  const { peaks, start, end, current, volume, color } = props

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement
    if (!parent) return
    const width = Math.max(1, parent.clientWidth)
    const height = 36
    const dpr = window.devicePixelRatio || 1
    canvas.width = Math.floor(width * dpr)
    canvas.height = Math.floor(height * dpr)
    canvas.style.width = `${width}px`
    canvas.style.height = `${height}px`
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, width, height)

    const mid = height / 2
    const gain = Math.min(2, Math.max(0, volume / 100))
    const barW = width / Math.max(peaks.length, 1)
    ctx.fillStyle = color
    for (let i = 0; i < peaks.length; i++) {
      const amp = Math.min(1, (peaks[i] ?? 0) * gain)
      const h = Math.max(amp > 0.004 ? amp * (height * 0.42) : 0, 0)
      if (h < 0.5) continue
      const x = i * barW
      const t = ((i + 0.5) / peaks.length) * duration
      ctx.globalAlpha = t >= start && t <= end ? 0.95 : 0.22
      ctx.fillRect(x, mid - h, Math.max(1, barW - 0.4), h * 2)
    }

    ctx.globalAlpha = 0.35
    ctx.fillStyle = token('--primary')
    const left = (start / duration) * width
    const right = (end / duration) * width
    ctx.fillRect(left, 0, Math.max(1, right - left), 2)
    ctx.fillRect(left, height - 2, Math.max(1, right - left), 2)

    ctx.globalAlpha = 1
    ctx.fillStyle = token('--rec')
    const playX = (current / duration) * width
    ctx.fillRect(playX, 0, 1, height)
  }, [peaks, start, end, current, volume, color, duration])

  return (
    <div
      className={cn(
        'h-9 overflow-hidden rounded-lg border border-border bg-background',
        props.disabled && 'pointer-events-none opacity-60'
      )}
      onPointerDown={(event) => {
        if (props.disabled) return
        const rect = event.currentTarget.getBoundingClientRect()
        const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
        props.onSeek(ratio * duration)
      }}
    >
      <canvas ref={canvasRef} className="block h-full w-full" />
    </div>
  )
}

class TrackMixer {
  private ctx: AudioContext | null = null
  private buffers = new Map<string, AudioBuffer>()
  private sources: AudioBufferSourceNode[] = []
  private gains: GainNode[] = []
  private loading: Promise<void> | null = null
  private gen = 0

  async start(tracks: ClipAudioTrack[], volumes: number[], offset: number): Promise<void> {
    const token = ++this.gen
    const ctx = this.ensure()
    await ctx.resume()
    await this.ensureBuffers(ctx, tracks)
    if (token !== this.gen) return
    this.stopSources()
    const when = ctx.currentTime
    const startAt = Math.max(0, offset)
    tracks.forEach((track, index) => {
      const buffer = this.buffers.get(track.previewUrl)
      if (!buffer) return
      const gain = ctx.createGain()
      gain.gain.value = Math.min(2, Math.max(0, (volumes[index] ?? 100) / 100))
      const source = ctx.createBufferSource()
      source.buffer = buffer
      source.connect(gain)
      gain.connect(ctx.destination)
      try {
        source.start(when, Math.min(startAt, Math.max(0, buffer.duration - 0.01)))
      } catch {
        return
      }
      this.gains[index] = gain
      this.sources.push(source)
    })
  }

  setGain(index: number, mul: number): void {
    const gain = this.gains[index]
    if (!gain) return
    gain.gain.value = Math.min(2, Math.max(0, mul))
  }

  stop(): void {
    this.gen += 1
    this.stopSources()
  }

  dispose(): void {
    this.gen += 1
    this.stopSources()
    this.buffers.clear()
    if (this.ctx) {
      void this.ctx.close()
      this.ctx = null
    }
  }

  private stopSources(): void {
    for (const source of this.sources) {
      try {
        source.stop()
      } catch {
        /* already stopped */
      }
      try {
        source.disconnect()
      } catch {
        /* already disconnected */
      }
    }
    for (const gain of this.gains) {
      try {
        gain.disconnect()
      } catch {
        /* already disconnected */
      }
    }
    this.sources = []
    this.gains = []
  }

  private ensure(): AudioContext {
    if (!this.ctx || this.ctx.state === 'closed') {
      this.ctx = new AudioContext()
    }
    return this.ctx
  }

  private async ensureBuffers(ctx: AudioContext, tracks: ClipAudioTrack[]): Promise<void> {
    if (this.loading) await this.loading
    const missing = tracks.filter((track) => !this.buffers.has(track.previewUrl))
    if (missing.length === 0) return
    this.loading = Promise.all(
      missing.map(async (track) => {
        const res = await fetch(track.previewUrl)
        if (!res.ok) throw new Error(`audio ${track.index} ${res.status}`)
        const raw = await res.arrayBuffer()
        const buffer = await ctx.decodeAudioData(raw.slice(0))
        this.buffers.set(track.previewUrl, buffer)
      })
    ).then(() => undefined)
    try {
      await this.loading
    } finally {
      this.loading = null
    }
  }
}
