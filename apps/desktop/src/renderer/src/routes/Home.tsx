import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router'
import { DEFAULT_HOTKEY, formatHotkey, type AppSettings } from '@echoframe/shared'
import { Loader2, Pause, Play, RotateCw } from 'lucide-react'
import { toast } from 'sonner'
import { PageHeader } from '@/components/PageHeader'
import { Button } from '@/components/ui/button'
import { Kbd, KbdGroup } from '@/components/ui/kbd'
import { eventToAccelerator } from '@/lib/accelerator'
import { recMode, useCaptureStatus, type RecMode } from '@/lib/capture'
import { formatAgo, formatBytes, formatDuration } from '@/lib/formats'
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
  recording: 'Recording in the background. Save a replay whenever you need it.',
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
  const [settings, setSettings] = useState<AppSettings | null>(null)
  const [clips, setClips] = useState<LibraryClip[]>([])
  const [clipsError, setClipsError] = useState<string | null>(null)
  const [settingsError, setSettingsError] = useState<string | null>(null)
  const [hotkey, setHotkey] = useState(DEFAULT_HOTKEY)
  const [rebinding, setRebinding] = useState(false)
  const [busy, setBusy] = useState(false)
  const [recent, setRecent] = useState<LibraryClip[]>([])
  const [justSaved, setJustSaved] = useState<LibraryClip | null>(null)
  const newestId = useRef<string | null | undefined>(undefined)
  const now = useNow(30_000)

  useEffect(() => {
    if (!hasDesktopApi()) return
    const loadSettings = (): void => {
      void ipc
        .getSettings()
        .then((value) => {
          setSettings(value)
          setSettingsError(null)
          setHotkey(value.hotkey)
        })
        .catch(() =>
          setSettingsError('Could not load capture settings. Reopen the dashboard to retry.')
        )
    }
    loadSettings()
    window.addEventListener('focus', loadSettings)
    const loadClips = (): void => {
      void ipc
        .listClips()
        .then((clips) => {
          const sorted = [...clips].sort((a, b) => b.createdAt - a.createdAt)
          const newest = sorted[0] ?? null
          // A clip that shows up while this page is open gets the "clip saved" lower third.
          if (newestId.current !== undefined && newest && newest.id !== newestId.current) {
            setJustSaved(newest)
          }
          newestId.current = newest?.id ?? null
          setClips(sorted)
          setRecent(sorted.slice(0, 4))
          setClipsError(null)
        })
        .catch(() => setClipsError('Could not load your clips. Reopen the dashboard to retry.'))
    }
    loadClips()
    const unsubscribe = ipc.onClipsChanged(loadClips)
    return () => {
      unsubscribe()
      window.removeEventListener('focus', loadSettings)
    }
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
  const sourceName = !capture?.ready
    ? 'Unavailable'
    : capturingGame
      ? target.title || target.exe
      : target?.kind === 'display'
        ? target.title || 'Display capture'
        : 'Waiting for a game'
  const hotkeyParts = formatHotkey(hotkey, hasDesktopApi() ? ipc.platform : 'win32').split(' + ')
  const last = recent[0] ?? null

  async function toggleBuffer(): Promise<void> {
    if (!hasDesktopApi() || busy) return
    setBusy(true)
    try {
      setCapture(capture?.paused ? await ipc.resumeBuffer() : await ipc.pauseBuffer())
    } catch (error) {
      toast.error('Recorder action failed', { description: String(error) })
    } finally {
      setBusy(false)
    }
  }

  async function restart(): Promise<void> {
    if (!hasDesktopApi() || busy) return
    setBusy(true)
    try {
      setCapture(await ipc.restartCapture())
    } catch (error) {
      toast.error('Recorder action failed', { description: String(error) })
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="flex h-full min-h-0 flex-col">
      <PageHeader
        title={titles[mode]}
        kicker="Replay dashboard"
        className="items-center px-6 pt-4 pb-2"
      >
        <div className="ml-auto flex items-center gap-2">
          <Button
            size="lg"
            className="min-w-[190px] justify-between"
            disabled={!live || capture?.saving}
            onClick={() => {
              if (!hasDesktopApi()) return
              void ipc
                .saveClip()
                .catch((error: unknown) =>
                  toast.error('Could not save clip', { description: String(error) })
                )
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
            <Button size="lg" variant="outline" disabled={busy} onClick={() => void restart()}>
              <RotateCw />
              Restart
            </Button>
          ) : (
            <Button
              size="lg"
              variant="outline"
              disabled={busy || mode === 'starting'}
              onClick={() => void toggleBuffer()}
            >
              {mode === 'paused' ? <Play /> : <Pause />}
              {mode === 'paused' ? 'Resume' : 'Pause'}
            </Button>
          )}
        </div>
      </PageHeader>
      <p className="shrink-0 px-6 pb-3 text-sm text-muted-foreground">{sublines[mode]}</p>
      {capture?.error || settingsError || clipsError ? (
        <p
          role="alert"
          className="mx-6 mb-3 max-h-12 shrink-0 overflow-y-auto border-l-2 border-destructive pl-3 text-sm text-destructive"
        >
          {capture?.error || settingsError || clipsError}
        </p>
      ) : null}
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_300px] gap-4 px-6 pb-4">
        <section className="card flex min-h-0 min-w-0 flex-col" aria-labelledby="recent-title">
          <div className="flex shrink-0 items-center justify-between px-4 py-2.5">
            <h2 id="recent-title" className="font-display text-lg font-bold">
              Latest replay
            </h2>
            <button
              type="button"
              className="text-xs text-muted-foreground hover:text-foreground"
              onClick={() => navigate('/library')}
            >
              All clips
            </button>
          </div>
          <LastReplay
            clip={last}
            now={now}
            justSaved={justSaved}
            hotkeyParts={hotkeyParts}
            onOpen={(clip) => navigate(`/editor/${clip.id}`)}
          />
          {recent.length > 1 ? (
            <ol className="shrink-0" aria-label="Earlier clips">
              {recent.slice(1).map((clip, index) => (
                <TowerRow
                  key={clip.id}
                  clip={clip}
                  index={index}
                  position={index + 2}
                  fresh={now - clip.createdAt < FRESH_MS}
                  now={now}
                  onOpen={() => navigate(`/editor/${clip.id}`)}
                />
              ))}
            </ol>
          ) : null}
        </section>
        <aside className="card flex min-h-0 min-w-0 flex-col" aria-labelledby="setup-title">
          <div className="flex shrink-0 items-center justify-between px-4 py-2.5">
            <h2 id="setup-title" className="font-display text-lg font-bold">
              Capture setup
            </h2>
            <button
              type="button"
              className="text-xs text-muted-foreground hover:text-foreground"
              onClick={() => navigate('/settings')}
            >
              Edit settings
            </button>
          </div>
          <dl className="mx-4 shrink-0 divide-y divide-border border-t border-border">
            <SetupRow label="Source" value={sourceName} />
            <SetupRow
              label="Replay"
              value={capture ? `${capture.bufferSec} seconds` : 'Unavailable'}
            />
            <SetupRow label="Output" value={outputDetail(capture) ?? 'Unavailable'} />
            <SetupRow label="Encoder" value={encoderName(capture?.encoderLabel ?? '')} />
            <SetupRow
              label="Microphone"
              value={
                settings
                  ? settings.micEnabled
                    ? `On / ${settings.micVolume}%`
                    : 'Off'
                  : 'Unavailable'
              }
            />
          </dl>
          <div className="mx-4 flex shrink-0 items-center justify-between gap-2 border-t border-border py-2.5">
            <span className="text-sm text-muted-foreground">Hotkey</span>
            <button
              type="button"
              onClick={() => setRebinding((value) => !value)}
              className="text-right text-xs text-highlight hover:underline"
            >
              {rebinding ? (
                'Press keys / Esc to cancel'
              ) : (
                <>
                  <span className="tc">{hotkeyParts.join(' + ')}</span> / Change
                </>
              )}
            </button>
          </div>
          <div className="mt-auto border-t border-border px-4 py-3">
            <p className="tc text-xs">
              {clips.length} clips /{' '}
              {formatBytes(
                clips.filter((clip) => clip.localFile).reduce((sum, clip) => sum + clip.bytes, 0)
              )}{' '}
              local
              {clips.some((clip) => clip.cloudStatus === 'uploading')
                ? ` / ${clips.filter((clip) => clip.cloudStatus === 'uploading').length} uploading`
                : ''}
            </p>
            <p
              className="mt-1 truncate text-xs text-muted-foreground"
              title={settings?.outputFolder}
            >
              Saved to {settings?.outputFolder ?? 'your clip folder'}
            </p>
          </div>
        </aside>
      </div>
    </main>
  )
}

/** The newest clip, framed like a TV replay: REPLAY bug, game, lower third. */
function LastReplay(props: {
  clip: LibraryClip | null
  now: number
  justSaved: LibraryClip | null
  hotkeyParts: string[]
  onOpen: (clip: LibraryClip) => void
}): React.JSX.Element {
  const clip = props.clip
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden bg-background">
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

      {props.justSaved ? (
        <div className="slide-in-left pointer-events-none absolute top-3 left-1/2 flex -translate-x-1/2 items-stretch">
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
          'grid w-full grid-cols-[22px_64px_minmax(0,1fr)] items-center gap-3 px-4 py-2 text-left transition-colors hover:bg-accent',
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
        <span className="relative aspect-video w-16 overflow-hidden bg-card">
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

function SetupRow(props: { label: string; value: string }): React.JSX.Element {
  return (
    <div className="grid grid-cols-[85px_minmax(0,1fr)] gap-3 py-2 text-sm">
      <dt className="text-muted-foreground">{props.label}</dt>
      <dd className="tc truncate text-right" title={props.value}>
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
