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

/** Clips saved within this window get the blue "new" bar in the list. */
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
        setRecent(sorted.slice(0, 6))
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
  const last = recent[0] ?? null

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

  return (
    <main className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_320px]">
      <section className="flex min-h-0 min-w-0 flex-col gap-4 overflow-y-auto px-7 pt-6 pb-5">
        <div className="flex items-end justify-between gap-6">
          <div key={mode} className="slide-in-left min-w-0">
            <h1 className="display-title text-[52px]">{titles[mode]}</h1>
            <p className="display-title mt-1 text-[22px] text-muted-foreground">{sublines[mode]}</p>
          </div>
          <div className="tc shrink-0 pb-1 text-right text-[12px] leading-[1.7] text-muted-foreground">
            <p>
              {encoderName(capture?.encoderLabel ?? '')} ·{' '}
              <span className="text-foreground">{outputDetail(capture) ?? '—'}</span>
            </p>
            <p>
              SOURCE · <span className="text-foreground">{sourceName}</span>
            </p>
          </div>
        </div>

        {capture?.error ? (
          <p className="border-l-2 border-destructive bg-destructive/10 px-3 py-2 text-[13px] text-destructive">
            {capture.error}
          </p>
        ) : null}

        <LastReplay
          clip={last}
          now={now}
          saving={mode === 'saving'}
          justSaved={justSaved}
          hotkeyParts={hotkeyParts}
          onOpen={(clip) => navigate(`/editor/${clip.id}`)}
        />

        <BufferRuler capture={capture} mode={mode} now={now} />

        <div className="flex items-center gap-3 pt-1">
          <Button
            size="xl"
            className="min-w-[220px] justify-between"
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
          <button
            type="button"
            onClick={() => setRebinding((value) => !value)}
            className={cn(
              'tc ml-auto text-right text-[12px] leading-[1.6] text-muted-foreground transition-colors hover:text-foreground',
              rebinding && 'text-highlight hover:text-highlight'
            )}
          >
            {rebinding ? (
              <>
                PRESS A NEW KEY COMBO…
                <br />
                ESC TO CANCEL
              </>
            ) : (
              <>
                WORKS IN-GAME, NO ALT-TAB
                <br />
                <span className="underline decoration-border-strong underline-offset-3">
                  CHANGE HOTKEY
                </span>
              </>
            )}
          </button>
        </div>
      </section>

      <aside className="flex min-h-0 flex-col border-l border-border bg-card">
        <div className="flex items-baseline justify-between px-4 pt-5 pb-3">
          <h2 className="display-title text-[24px]">Latest clips</h2>
          <button
            type="button"
            onClick={() => navigate('/library')}
            className="eyebrow transition-colors hover:text-foreground"
          >
            All clips →
          </button>
        </div>
        <ol className="min-h-0 flex-1 overflow-y-auto">
          {recent.length > 0 ? (
            recent.map((clip, index) => (
              <TowerRow
                key={clip.id}
                clip={clip}
                index={index}
                position={recent.length - index}
                fresh={now - clip.createdAt < FRESH_MS}
                now={now}
                onOpen={() => navigate(`/editor/${clip.id}`)}
              />
            ))
          ) : (
            <li className="border-t border-border px-4 py-6 text-[12.5px] text-muted-foreground">
              Your clips show up here, newest first.
            </li>
          )}
        </ol>
        <dl className="grid grid-cols-2 border-t border-border">
          <SysCell label="Buffer" value={`${capture?.bufferSec ?? 60} s`} />
          <SysCell label="Quality" value={outputDetail(capture) ?? '—'} />
          <SysCell label="Encoder" value={encoderName(capture?.encoderLabel ?? '')} />
          <SysCell label={capturingGame ? 'Game' : 'Source'} value={sourceName} />
        </dl>
      </aside>
    </main>
  )
}

/** The newest clip, framed like a TV replay: REPLAY bug, game, lower third. */
function LastReplay(props: {
  clip: LibraryClip | null
  now: number
  saving: boolean
  justSaved: LibraryClip | null
  hotkeyParts: string[]
  onOpen: (clip: LibraryClip) => void
}): React.JSX.Element {
  const clip = props.clip
  return (
    <div className="relative aspect-video max-h-[46vh] min-h-[180px] w-full shrink-0 self-start overflow-hidden bg-black outline-1 outline-border-strong">
      {clip ? (
        <button
          type="button"
          onClick={() => props.onOpen(clip)}
          className="group absolute inset-0 text-left"
        >
          {clip.thumbUrl ? (
            <img
              key={clip.id}
              src={clip.thumbUrl}
              alt=""
              className="absolute inset-0 size-full object-cover transition-transform duration-500 group-hover:scale-[1.02]"
            />
          ) : null}
          <span className="absolute inset-0 bg-[linear-gradient(180deg,var(--scrim),transparent_24%,transparent_58%,var(--scrim))]" />
          <span className="bug absolute top-3 left-3">
            <span>REPLAY</span>
            <span>{formatDuration(clip.durationMs)}</span>
          </span>
          {clip.gameTitle ? (
            <span className="absolute top-3 right-3 bg-[var(--scrim)] px-2.5 py-1 font-display text-[13px] font-semibold">
              {clip.gameTitle}
            </span>
          ) : null}
          <span key={clip.id} className="slide-in-left absolute right-3 bottom-3 left-3 block">
            <span className="eyebrow block text-foreground/70">
              Last clip · {formatAgo(clip.createdAt, props.now)}
            </span>
            <span className="display-title mt-1 block truncate text-[28px]">{clip.title}</span>
          </span>
          <span className="absolute right-3 bottom-3 bg-primary px-3 py-1.5 font-display text-[13px] font-bold opacity-0 transition-opacity group-hover:opacity-100">
            Open in editor
          </span>
        </button>
      ) : (
        <div className="absolute inset-0 grid place-items-center bg-card">
          <div className="text-center">
            <span className="bug">
              <span>REPLAY</span>
              <span>--:--</span>
            </span>
            <p className="display-title mt-4 text-[26px]">No replays yet.</p>
            <p className="mt-1 text-[13px] text-muted-foreground">
              Play something, then press{' '}
              <span className="tc text-foreground">{props.hotkeyParts.join(' + ')}</span>.
            </p>
          </div>
        </div>
      )}

      {props.saving ? (
        <span
          aria-hidden
          className="flash-save pointer-events-none absolute inset-0 bg-foreground"
        />
      ) : null}

      {props.justSaved ? (
        <div className="slide-in-left pointer-events-none absolute top-3 left-1/2 flex -translate-x-1/2 items-stretch shadow-[0_12px_30px_var(--shadow)]">
          <span className="grid place-items-center bg-primary px-3 font-display text-[22px] font-black">
            +{formatDuration(props.justSaved.durationMs)}
          </span>
          <span className="bg-popover px-4 py-2">
            <span className="eyebrow block">Clip saved</span>
            <span className="display-title block max-w-[300px] truncate text-[19px]">
              {props.justSaved.title}
            </span>
          </span>
        </div>
      ) : null}
    </div>
  )
}

/**
 * The rolling buffer as a strip of one-second cells. It fills from the left
 * after a (re)start; once full, a marker every ten seconds slides left so you
 * can see time passing through it. The red line is now.
 */
function BufferRuler(props: {
  capture: CaptureStatus | null
  mode: RecMode
  now: number
}): React.JSX.Element {
  const total = props.capture?.bufferSec ?? 60
  const cells = Math.min(total, 60)
  const perCell = total / cells
  const since = props.capture?.bufferedSince ?? null
  const live = props.mode === 'recording' || props.mode === 'saving'
  const filledSec = live && since ? Math.min(total, (props.now - since) / 1000) : 0
  const filled = Math.floor(filledSec / perCell)
  const shift = Math.floor(props.now / 1000 / perCell)
  const labels = rulerLabels(total)

  return (
    <div className="shrink-0">
      <div className="mb-2 flex items-baseline justify-between">
        <p className="eyebrow">
          In buffer ·{' '}
          <span className={live ? 'text-foreground' : undefined}>
            {formatClock(filledSec)} / {formatClock(total)}
          </span>
        </p>
        <p className="eyebrow">{live ? 'Rolling' : props.mode === 'paused' ? 'Paused' : '—'}</p>
      </div>
      <div className="relative flex h-10 gap-[2px] pr-1">
        {Array.from({ length: cells }, (_, index) => {
          // index 0 is the oldest second, the last cell is now
          const age = cells - 1 - index
          const on = age < filled
          const marker = on && filled >= cells && (index + shift) % 10 === 0
          return (
            <span
              key={on && age === 0 ? `lead-${filled}` : index}
              className={cn(
                'flex-1 transition-colors duration-300',
                on ? 'bg-primary' : 'bg-secondary',
                on && age === 0 && filled < cells && 'cell-fill',
                marker && 'bg-highlight'
              )}
              style={on ? { opacity: 0.6 + 0.4 * (1 - age / cells) } : undefined}
            />
          )
        })}
        <span className="absolute -top-1.5 -bottom-1.5 right-0 w-0.5 bg-rec" />
      </div>
      <div className="tc mt-2 flex justify-between text-[11.5px] text-muted-foreground">
        {labels.map((label) => (
          <span key={label}>−{label}</span>
        ))}
        <span className="text-rec">NOW</span>
      </div>
    </div>
  )
}

/** Six evenly spaced "seconds ago" labels for the ruler, oldest first. */
function rulerLabels(total: number): string[] {
  const steps = 6
  return Array.from({ length: steps }, (_, index) => {
    const seconds = Math.round(total - (total / steps) * index)
    return seconds >= 120 && seconds % 60 === 0 ? `${seconds / 60}m` : `${seconds}`
  })
}

/** One clip in the list, like a row in a race timing tower. */
function TowerRow(props: {
  clip: LibraryClip
  index: number
  position: number
  fresh: boolean
  now: number
  onOpen: () => void
}): React.JSX.Element {
  const clip = props.clip
  return (
    <li
      className="rise-in border-t border-border"
      style={{ '--i': props.index } as React.CSSProperties}
    >
      <button
        type="button"
        onClick={props.onOpen}
        className={cn(
          'grid w-full grid-cols-[22px_80px_minmax(0,1fr)] items-center gap-3 px-4 py-2 text-left transition-colors hover:bg-accent',
          props.fresh && 'bg-popover shadow-[inset_3px_0_0_var(--primary)]'
        )}
      >
        <span
          className={cn(
            'tc text-[12px] font-semibold',
            props.fresh ? 'text-primary' : 'text-muted-foreground'
          )}
        >
          {props.position.toString().padStart(2, '0')}
        </span>
        <span className="relative aspect-video w-20 overflow-hidden bg-black">
          {clip.thumbUrl ? (
            <img src={clip.thumbUrl} alt="" className="size-full object-cover" />
          ) : null}
          <span className="tc absolute right-0.5 bottom-0.5 bg-[var(--scrim)] px-1 text-[11px]">
            {formatDuration(clip.durationMs)}
          </span>
        </span>
        <span className="min-w-0">
          <span className="block truncate font-display text-[15px] leading-tight font-bold">
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
        </span>
      </button>
    </li>
  )
}

function SysCell(props: { label: string; value: string }): React.JSX.Element {
  return (
    <div className="min-w-0 border-border px-4 py-2.5 odd:border-r [&:nth-child(-n+2)]:border-b">
      <dt className="eyebrow">{props.label}</dt>
      <dd className="truncate font-display text-[16px] font-bold" title={props.value}>
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
