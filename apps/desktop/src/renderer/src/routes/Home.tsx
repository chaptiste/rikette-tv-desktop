import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { DEFAULT_HOTKEY, formatHotkey } from '@echoframe/shared'
import { Loader2, Pause, Play, RotateCw } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Kbd, KbdGroup } from '@/components/ui/kbd'
import { eventToAccelerator } from '@/lib/accelerator'
import { recMode, useCaptureStatus, type RecMode } from '@/lib/capture'
import { formatAgo, formatClock, formatDuration } from '@/lib/formats'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import type { LibraryClip, CaptureStatus } from '../../../tauri/api'

const titles: Record<RecMode, string> = {
  recording: 'Rolling.',
  paused: 'Paused.',
  starting: 'Warming up.',
  saving: 'Cutting…',
  offline: 'Off air.'
}

const sublines: Record<RecMode, string> = {
  recording: 'Nothing gets lost. Hit the key when it happens.',
  paused: 'The buffer is empty until you resume.',
  starting: 'Starting the replay buffer.',
  saving: 'Writing the last seconds to disk.',
  offline: 'The recorder is not running.'
}

/** Clips saved within this window get the blue outline and NEW tag. */
const FRESH_MS = 2 * 60_000

export function Home(): React.JSX.Element {
  const navigate = useNavigate()
  const [capture, setCapture] = useCaptureStatus()
  const [hotkey, setHotkey] = useState(DEFAULT_HOTKEY)
  const [rebinding, setRebinding] = useState(false)
  const [busy, setBusy] = useState(false)
  const [recent, setRecent] = useState<LibraryClip[]>([])
  const [justSaved, setJustSaved] = useState<LibraryClip | null>(null)
  const newestId = useRef<string | null | undefined>(undefined)
  const now = useNow(1000)

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getSettings().then((settings) => setHotkey(settings.hotkey))
    const loadClips = (): void => {
      void ipc.listClips().then((clips) => {
        const sorted = [...clips].sort((a, b) => b.createdAt - a.createdAt)
        const newest = sorted[0] ?? null
        // A clip that shows up while this page is open gets the "clip saved" lower third.
        if (newestId.current !== undefined && newest && newest.id !== newestId.current) {
          setJustSaved(newest)
        }
        newestId.current = newest?.id ?? null
        setRecent(sorted.slice(0, 8))
      })
    }
    loadClips()
    return ipc.onClipsChanged(loadClips)
  }, [])

  useEffect(() => {
    if (!justSaved) return
    const timer = window.setTimeout(() => setJustSaved(null), 3600)
    return () => window.clearTimeout(timer)
  }, [justSaved])

  useEffect(() => {
    if (!rebinding) return
    const onKeyDown = (event: KeyboardEvent): void => {
      event.preventDefault()
      event.stopPropagation()
      if (event.key === 'Escape') {
        setRebinding(false)
        return
      }
      const accelerator = eventToAccelerator(event, hasDesktopApi() ? ipc.platform : 'win32')
      if (!accelerator) return
      setRebinding(false)
      void ipc
        .updateSettings({ hotkey: accelerator })
        .then((result) => setHotkey(result.settings.hotkey))
        .catch((error: unknown) => {
          toast.error('Could not rebind hotkey', {
            description: error instanceof Error ? error.message : String(error)
          })
        })
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [rebinding])

  const mode = recMode(capture)
  const live = mode === 'recording' || mode === 'saving'
  const target = capture?.captureTarget
  const capturingGame = target?.kind === 'game' && Boolean(target.exe)
  const sourceName = capturingGame ? target.title || target.exe : 'Desktop'
  const hotkeyParts = formatHotkey(hotkey, hasDesktopApi() ? ipc.platform : 'win32').split(' + ')

  async function toggleBuffer(): Promise<void> {
    if (!hasDesktopApi() || busy) return
    setBusy(true)
    try {
      setCapture(capture?.paused ? await ipc.resumeBuffer() : await ipc.pauseBuffer())
    } finally {
      setBusy(false)
    }
  }

  async function restart(): Promise<void> {
    if (!hasDesktopApi() || busy) return
    setBusy(true)
    try {
      setCapture(await ipc.restartCapture())
    } finally {
      setBusy(false)
    }
  }

  const total = capture?.bufferSec ?? 60
  const since = capture?.bufferedSince ?? null
  const held = live && since ? Math.min(total, (now - since) / 1000) : 0

  return (
    <main className="flex h-full min-h-0 flex-col gap-6 overflow-y-auto px-7 pt-6 pb-6">
      <section className="card shrink-0 border-t-2 border-t-primary">
        <div className="flex items-center justify-between gap-6 px-6 pt-5 pb-5">
          <div key={mode} className="slide-in-left min-w-0">
            <p className="eyebrow flex items-center gap-2">
              <span className={cn('live-dot', live ? 'text-rec' : 'text-muted-foreground')} />
              Replay buffer
            </p>
            <h1 className="display-title mt-2 text-[48px]">{titles[mode]}</h1>
            <p className="mt-1 text-[13px] text-muted-foreground">{sublines[mode]}</p>
          </div>
          <div className="flex shrink-0 flex-col items-end gap-3">
            <div className="flex items-center gap-3">
              {mode === 'offline' ? (
                <Button size="xl" variant="outline" disabled={busy} onClick={() => void restart()}>
                  <RotateCw />
                  Restart
                </Button>
              ) : (
                <Button
                  size="xl"
                  variant="outline"
                  disabled={busy || mode === 'starting'}
                  onClick={() => void toggleBuffer()}
                >
                  {mode === 'paused' ? <Play /> : <Pause />}
                  {mode === 'paused' ? 'Resume' : 'Pause'}
                </Button>
              )}
              <Button
                size="xl"
                className="min-w-[200px] justify-between"
                disabled={!live || capture?.saving}
                onClick={() => {
                  if (!hasDesktopApi()) return
                  void ipc.saveClip()
                }}
              >
                {capture?.saving ? (
                  <>
                    Saving <Loader2 className="animate-spin" />
                  </>
                ) : (
                  <>
                    Clip it
                    <KbdGroup>
                      {hotkeyParts.map((part, index) => (
                        <Kbd key={`${part}-${index}`} className="kbd-on-primary">
                          {part}
                        </Kbd>
                      ))}
                    </KbdGroup>
                  </>
                )}
              </Button>
            </div>
            <button
              type="button"
              onClick={() => setRebinding((value) => !value)}
              className={cn(
                'eyebrow transition-colors hover:text-foreground',
                rebinding && 'text-highlight hover:text-highlight'
              )}
            >
              {rebinding ? (
                'Press a new key combo… Esc to cancel'
              ) : (
                <>
                  Works in-game ·{' '}
                  <span className="underline decoration-border-strong underline-offset-3">
                    change hotkey
                  </span>
                </>
              )}
            </button>
          </div>
        </div>

        {capture?.error ? (
          <p className="mx-6 mb-5 border-l-2 border-destructive bg-destructive/10 px-3 py-2 text-[13px] text-destructive">
            {capture.error}
          </p>
        ) : null}

        <dl className="grid grid-cols-4 border-t border-border">
          <Readout
            label="In buffer"
            value={`${formatClock(held)} / ${formatClock(total)}`}
            dim={!live}
          />
          <Readout label={capturingGame ? 'Game' : 'Source'} value={sourceName} />
          <Readout label="Video" value={outputDetail(capture) ?? '—'} />
          <Readout label="Encoder" value={encoderName(capture?.encoderLabel ?? '')} />
        </dl>
      </section>

      <section className="flex min-h-0 flex-col">
        <div className="mb-3 flex items-baseline justify-between">
          <h2 className="display-title text-[26px]">Recent clips</h2>
          <button
            type="button"
            onClick={() => navigate('/library')}
            className="eyebrow transition-colors hover:text-foreground"
          >
            All clips →
          </button>
        </div>
        {recent.length > 0 ? (
          <ol className="grid grid-cols-4 gap-4">
            {recent.map((clip, index) => (
              <ClipCard
                key={clip.id}
                clip={clip}
                index={index}
                fresh={now - clip.createdAt < FRESH_MS}
                saved={justSaved?.id === clip.id}
                flash={index === 0 && mode === 'saving'}
                now={now}
                onOpen={() => navigate(`/editor/${clip.id}`)}
              />
            ))}
          </ol>
        ) : (
          <div className="card grid place-items-center px-6 py-14 text-center">
            <span className="bug">
              <span>REPLAY</span>
              <span>--:--</span>
            </span>
            <p className="display-title mt-4 text-[26px]">No clips yet.</p>
            <p className="mt-1 text-[13px] text-muted-foreground">
              Play something, then press{' '}
              <span className="tc text-foreground">{hotkeyParts.join(' + ')}</span>.
            </p>
          </div>
        )}
      </section>
    </main>
  )
}

/** A recent clip: thumbnail with duration, title, game and age. */
function ClipCard(props: {
  clip: LibraryClip
  index: number
  fresh: boolean
  saved: boolean
  flash: boolean
  now: number
  onOpen: () => void
}): React.JSX.Element {
  const clip = props.clip
  return (
    <li className="rise-in" style={{ '--i': props.index } as React.CSSProperties}>
      <button type="button" onClick={props.onOpen} className="group block w-full text-left">
        <span
          className={cn(
            'relative block aspect-video overflow-hidden bg-black outline-1 outline-border transition-[outline-color] duration-150 group-hover:outline-2 group-hover:outline-primary',
            props.fresh && 'outline-2 outline-primary'
          )}
        >
          {clip.thumbUrl ? (
            <img src={clip.thumbUrl} alt="" className="absolute inset-0 size-full object-cover" />
          ) : null}
          {props.fresh ? (
            <span className="bug bug-blue absolute top-2 left-2">
              <span>NEW</span>
              <span>{formatAgo(clip.createdAt, props.now)}</span>
            </span>
          ) : null}
          <span className="tc absolute right-2 bottom-2 bg-[var(--scrim)] px-1.5 py-0.5 text-[11px]">
            {formatDuration(clip.durationMs)}
          </span>
          <span className="absolute bottom-2 left-2 bg-primary px-2 py-1 text-[12px] font-medium text-primary-foreground opacity-0 transition-opacity group-hover:opacity-100">
            Open in editor
          </span>
          {props.flash ? (
            <span
              aria-hidden
              className="flash-save pointer-events-none absolute inset-0 bg-foreground"
            />
          ) : null}
          {props.saved ? (
            <span className="slide-in-left pointer-events-none absolute right-2 bottom-2 left-2 flex items-stretch shadow-[0_12px_30px_var(--shadow)]">
              <span className="grid place-items-center bg-primary px-2.5 font-display text-[18px] font-bold text-primary-foreground">
                +{formatDuration(clip.durationMs)}
              </span>
              <span className="min-w-0 bg-popover px-3 py-1.5">
                <span className="eyebrow block">Clip saved</span>
                <span className="display-title block truncate text-[16px]">{clip.title}</span>
              </span>
            </span>
          ) : null}
        </span>
        <span className="mt-2 block truncate font-display text-[15px] leading-tight font-semibold">
          {clip.title}
        </span>
        <span className="tc mt-1 block truncate text-[12px] text-muted-foreground">
          {[clip.gameTitle, formatAgo(clip.createdAt, props.now)].filter(Boolean).join(' · ')}
          {clip.cloudStatus === 'uploading' ? (
            <span className="text-highlight"> · uploading</span>
          ) : clip.cloudStatus === 'live' ? (
            <span className="text-ok"> · link</span>
          ) : null}
        </span>
      </button>
    </li>
  )
}

function Readout(props: { label: string; value: string; dim?: boolean }): React.JSX.Element {
  return (
    <div className="min-w-0 border-border px-6 py-3 not-last:border-r">
      <dt className="eyebrow">{props.label}</dt>
      <dd
        className={cn(
          'tc mt-0.5 truncate text-[15px] font-medium',
          props.dim ? 'text-muted-foreground' : 'text-foreground'
        )}
        title={props.value}
      >
        {props.value}
      </dd>
    </div>
  )
}

/** Re-renders every `ms` so timers and "x min ago" stay current. */
function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), ms)
    return () => window.clearInterval(timer)
  }, [ms])
  return now
}

function encoderName(label: string): string {
  if (/nvenc/i.test(label)) return 'NVENC'
  if (/amf/i.test(label)) return 'AMF'
  if (/qsv|quick sync/i.test(label)) return 'QSV'
  if (/videotoolbox/i.test(label)) return 'VideoToolbox'
  if (/x264/i.test(label)) return 'x264'
  return label || '—'
}

function outputDetail(capture: CaptureStatus | null): string | undefined {
  if (!capture?.outputHeight) return undefined
  const height = capture.outputHeight
  const standard = height === 2160 || height === 1440 || height === 1080 || height === 720
  return standard ? `${height}p${capture.fps}` : `${capture.outputWidth}×${height}`
}
