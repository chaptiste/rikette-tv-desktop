import {
  type ComponentProps,
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState
} from 'react'
import { useNavigate, useParams } from 'react-router'
import {
  ArrowLeftToLine,
  ArrowRightToLine,
  ChevronLeft,
  FileX,
  FolderOpen,
  Loader2,
  MoreHorizontal,
  Pause,
  Play,
  Repeat,
  Share,
  Trash2,
  Upload
} from 'lucide-react'
import { Alert, AlertDescription } from '@/components/ui/alert'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/components/ui/dropdown-menu'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { ClipTitle } from '@/components/ClipTitle'
import { PageHeader } from '@/components/PageHeader'
import { formatBytes, formatDuration, formatTimecode } from '@/lib/formats'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import { AudioLanes } from './AudioLanes'
import { TrimBar } from './TrimBar'
import type { ClipAudioTrack, LibraryClip } from '../../../tauri/api'

const NO_TRACKS: ClipAudioTrack[] = []
const NO_VOLUMES: number[] = []

export function Editor(): React.JSX.Element {
  const { clipId } = useParams()
  const navigate = useNavigate()
  const videoRef = useRef<HTMLVideoElement>(null)
  const [clip, setClip] = useState<LibraryClip | null>(null)
  const [missing, setMissing] = useState(false)
  const [duration, setDuration] = useState(0)
  const [start, setStart] = useState(0)
  const [end, setEnd] = useState(0)
  const [current, setCurrent] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [loopSelection, setLoopSelection] = useState(true)
  const [mediaHeld, setMediaHeld] = useState(true)
  const [overwriteOpen, setOverwriteOpen] = useState(false)
  // Tracks are read per file; a stale entry (other clip, or before a trim) reads as none.
  const [audio, setAudio] = useState<{
    key: string
    tracks: ClipAudioTrack[]
    volumes: number[]
  } | null>(null)
  const audioKey = clip?.localFile && hasDesktopApi() ? `${clip.id}|${clip.videoUrl}` : null
  const clipIdForAudio = clip?.id
  const tracks = audio && audio.key === audioKey ? audio.tracks : NO_TRACKS
  const volumes = audio && audio.key === audioKey ? audio.volumes : NO_VOLUMES
  const [seekNonce, setSeekNonce] = useState(0)
  const [signedIn, setSignedIn] = useState(false)
  const audioSeekTimer = useRef(0)
  // Any running Discord export, from this clip or another one started in the Library.
  const [discordExport, setDiscordExport] = useState<{ id: string; percent: number } | null>(null)

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getMe().then((me) => setSignedIn(Boolean(me)))
    return ipc.onAuthChanged((me) => setSignedIn(Boolean(me)))
  }, [])

  useEffect(() => {
    if (!hasDesktopApi()) return
    return ipc.onExportProgress(({ id, percent }) => {
      setDiscordExport(percent === null ? null : { id, percent })
    })
  }, [])

  useEffect(() => {
    if (!clipId || !hasDesktopApi()) return
    let cancelled = false
    void ipc.getClip(clipId).then((next) => {
      if (cancelled) return
      applyClip(
        next,
        setClip,
        setMissing,
        setDuration,
        setStart,
        setEnd,
        setCurrent,
        setPlaying,
        setError
      )
    })
    const off = ipc.onClipsChanged(() => {
      void ipc.getClip(clipId).then((next) => {
        if (cancelled) return
        if (!next) {
          setMissing(true)
          setClip(null)
          return
        }
        setClip(next)
      })
    })
    return () => {
      cancelled = true
      off()
    }
  }, [clipId])

  useEffect(() => {
    if (!audioKey || !clipIdForAudio) return
    let cancelled = false
    void ipc
      .getClipAudio(clipIdForAudio)
      .then((next) => {
        if (!cancelled) setAudio({ key: audioKey, tracks: next, volumes: next.map(() => 100) })
      })
      .catch(() => {
        if (!cancelled) setAudio({ key: audioKey, tracks: [], volumes: [] })
      })
    return () => {
      cancelled = true
    }
  }, [audioKey, clipIdForAudio])

  const bumpAudio = (): void => {
    window.clearTimeout(audioSeekTimer.current)
    setSeekNonce((value) => value + 1)
  }

  const seek = (time: number): void => {
    const video = videoRef.current
    if (!video) return
    video.currentTime = time
    setCurrent(time)
    if (!video.paused) {
      window.clearTimeout(audioSeekTimer.current)
      audioSeekTimer.current = window.setTimeout(() => setSeekNonce((value) => value + 1), 90)
    }
  }

  const playFrom = useCallback((time: number): void => {
    const video = videoRef.current
    if (!video) return
    const startPlayback = (): void => {
      void video.play()
    }
    if (Math.abs(video.currentTime - time) < 0.04 && !video.seeking) {
      startPlayback()
      return
    }
    const onSeeked = (): void => {
      window.clearTimeout(timer)
      video.removeEventListener('seeked', onSeeked)
      startPlayback()
    }
    const timer = window.setTimeout(onSeeked, 350)
    video.addEventListener('seeked', onSeeked)
    video.currentTime = time
    setCurrent(time)
  }, [])

  const togglePlay = useCallback((): void => {
    const video = videoRef.current
    if (!video) return
    if (!video.paused) {
      video.pause()
      return
    }
    const inside = video.currentTime >= start - 0.04 && video.currentTime < end - 0.04
    playFrom(inside ? video.currentTime : start)
  }, [end, playFrom, start])

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const tag = (event.target as HTMLElement | null)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      if (event.code === 'Space') {
        event.preventDefault()
        togglePlay()
      } else if (clip?.localFile && (event.key === 'i' || event.key === 'I')) {
        setStart(Math.min(current, end - 0.25))
      } else if (clip?.localFile && (event.key === 'o' || event.key === 'O')) {
        setEnd(Math.max(current, start + 0.25))
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [clip?.localFile, current, end, start, togglePlay])

  const runTrim = async (mode: 'overwrite' | 'save-as-new'): Promise<void> => {
    if (!clip || !hasDesktopApi()) return
    setBusy(true)
    setError(null)
    setMediaHeld(false)
    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)))
    try {
      const next = await ipc.trimClip({
        id: clip.id,
        startSec: start,
        endSec: end,
        mode,
        volumes: volumes.map((value) => value / 100)
      })
      if (mode === 'save-as-new') {
        navigate(`/editor/${next.id}`, { replace: true })
        return
      }
      applyClip(
        next,
        setClip,
        setMissing,
        setDuration,
        setStart,
        setEnd,
        setCurrent,
        setPlaying,
        setError
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setMediaHeld(true)
      setBusy(false)
    }
  }

  const runDiscordExport = (): void => {
    if (!clip || !hasDesktopApi()) return
    setDiscordExport({ id: clip.id, percent: 0 })
    // The main process reports failures with a toast.
    void ipc
      .exportForDiscord({
        id: clip.id,
        startSec: start,
        endSec: end,
        volumes: volumes.map((value) => value / 100)
      })
      .catch(() => undefined)
  }

  if (!clipId) {
    return (
      <main className="flex h-full flex-col">
        <PageHeader kicker="Cutting room" title="Editor" />
        <EditorEmpty
          title="No clip loaded"
          body="Pick a clip to trim, mix and share."
          onBack={() => navigate('/library')}
        />
      </main>
    )
  }

  if (missing) {
    return (
      <main className="flex h-full flex-col">
        <PageHeader kicker="Cutting room" title="Editor" />
        <EditorEmpty
          title="Clip is gone"
          body="It was deleted or moved."
          onBack={() => navigate('/library')}
        />
      </main>
    )
  }

  if (!clip) {
    return (
      <main className="flex h-full flex-col">
        <PageHeader kicker="Cutting room" title="Editor" />
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="size-8 animate-spin text-highlight" strokeWidth={2} />
        </div>
      </main>
    )
  }

  const selection = Math.max(0, end - start)
  const cloudOnly = !clip.localFile
  const volumeDirty = volumes.some((value) => Math.abs(value - 100) > 1)
  const trimmed = selection < duration - 0.05
  const canExport = Boolean(clip.localFile) && !busy && (volumeDirty || trimmed)
  const exportHint = busy ? 'Saving…' : trimmed || volumeDirty ? undefined : 'Trim the clip to save'

  return (
    <main className="flex h-full flex-col">
      <PageHeader className="px-6">
        <Hint label="Clips">
          <Button
            variant="secondary"
            size="icon"
            className="shrink-0 [&_svg]:size-4"
            onClick={() => navigate('/library')}
          >
            <ChevronLeft strokeWidth={2} />
          </Button>
        </Hint>
        <div className="flex min-w-0 max-w-[min(36rem,60%)] flex-col">
          {clip.gameTitle ? (
            <span className="eyebrow truncate">{clip.gameTitle}</span>
          ) : (
            <span className="eyebrow">Cutting room</span>
          )}
          <ClipTitle
            id={clip.id}
            title={clip.title}
            renameWith="icon"
            showRenameIcon="always"
            className="[&>span]:text-[24px] [&>span]:font-extrabold [&>span]:[font-stretch:72%]"
            onRenamed={(title) => setClip({ ...clip, title })}
          />
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-2">
          <MetaChip>{formatDuration(clip.durationMs)}</MetaChip>
          <MetaChip>
            {clip.width}×{clip.height}
          </MetaChip>
          <MetaChip>{clip.localFile ? formatBytes(clip.bytes) : 'Online'}</MetaChip>
        </div>
      </PageHeader>

      <div className="flex min-h-0 flex-1 items-center justify-center px-8 pb-4">
        <video
          ref={videoRef}
          key={clip.videoUrl}
          src={mediaHeld ? clip.videoUrl : undefined}
          preload="auto"
          muted={tracks.length > 0}
          className="max-h-full max-w-full bg-black outline-1 outline-border-strong"
          onClick={togglePlay}
          onTimeUpdate={() => {
            const video = videoRef.current
            if (!video || video.seeking) return
            const t = video.currentTime
            if (
              clip.localFile &&
              loopSelection &&
              !video.paused &&
              (t < start - 0.05 || t >= end - 0.04)
            ) {
              video.currentTime = start
              setCurrent(start)
              bumpAudio()
              return
            }
            if (clip.localFile && !loopSelection && t >= end - 0.04) {
              video.pause()
              video.currentTime = end
              setCurrent(end)
              return
            }
            setCurrent(t)
          }}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onLoadedMetadata={(event) => {
            const seconds = event.currentTarget.duration
            if (Number.isFinite(seconds) && seconds > 0) {
              setDuration(seconds)
              setEnd((prev) => (prev <= 0 || prev > seconds ? seconds : prev))
            }
          }}
        />
      </div>

      <section className="relative border-t border-border bg-sidebar">
        <div className="mx-auto flex w-full max-w-[96rem] flex-col gap-3 px-8 pt-3 pb-3">
          {cloudOnly ? (
            <p className="text-center text-[12px] text-muted-foreground">
              Playing the cloud copy — compressed, not original quality. Trimming needs a local
              file.
            </p>
          ) : (
            <div className="card overflow-hidden">
              <div
                className={
                  tracks.length > 0
                    ? 'grid grid-cols-[4.75rem_minmax(0,1fr)_7.25rem] items-center gap-x-2 gap-y-1.5 px-2.5 py-2.5'
                    : 'px-2.5 pt-2.5 pb-2'
                }
              >
                {tracks.length > 0 ? (
                  <div className="min-w-0">
                    <div className="eyebrow truncate text-foreground">Video</div>
                  </div>
                ) : null}
                <TrimBar
                  duration={duration}
                  start={start}
                  end={end}
                  current={current}
                  disabled={busy}
                  onStart={(value) => {
                    setStart(value)
                    seek(value)
                  }}
                  onEnd={(value) => {
                    setEnd(value)
                    seek(value)
                  }}
                  onSeek={seek}
                />
                {tracks.length > 0 ? <div /> : null}
                <AudioLanes
                  tracks={tracks}
                  volumes={volumes}
                  duration={duration}
                  start={start}
                  end={end}
                  current={current}
                  playing={playing}
                  seekNonce={seekNonce}
                  mediaHeld={mediaHeld}
                  disabled={busy}
                  onVolume={(index, percent) => {
                    setAudio((prev) =>
                      prev
                        ? {
                            ...prev,
                            volumes: prev.volumes.map((value, i) => (i === index ? percent : value))
                          }
                        : prev
                    )
                  }}
                  onSeek={seek}
                />
              </div>
              <div className="grid grid-cols-4 border-t border-border">
                <TimeStat
                  label="In"
                  value={formatTimecode(start)}
                  hint="Jump to in"
                  shortcut="I sets from playhead"
                  onClick={() => seek(start)}
                />
                <TimeStat
                  label="Out"
                  value={formatTimecode(end)}
                  hint="Jump to out"
                  shortcut="O sets from playhead"
                  onClick={() => seek(end)}
                />
                <TimeStat label="Selected" value={formatTimecode(selection)} accent />
                <TimeStat label="Playhead" value={formatTimecode(current)} muted />
              </div>
            </div>
          )}

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-0.5">
              {cloudOnly ? null : (
                <IconBtn
                  variant="ghost"
                  label="Jump to in"
                  disabled={busy}
                  className="size-8 [&_svg]:size-4"
                  onClick={() => seek(start)}
                >
                  <ArrowLeftToLine strokeWidth={1.5} />
                </IconBtn>
              )}
              <IconBtn
                label={playing ? 'Pause' : 'Play'}
                shortcut="Space"
                disabled={busy}
                className="mx-1 h-10 w-12 [&_svg]:size-4"
                onClick={togglePlay}
              >
                {playing ? (
                  <Pause className="fill-current" strokeWidth={2} />
                ) : (
                  <Play className="translate-x-px fill-current" strokeWidth={2} />
                )}
              </IconBtn>
              {cloudOnly ? null : (
                <>
                  <IconBtn
                    variant="ghost"
                    label="Jump to out"
                    disabled={busy}
                    className="size-8 [&_svg]:size-4"
                    onClick={() => seek(end)}
                  >
                    <ArrowRightToLine strokeWidth={1.5} />
                  </IconBtn>
                  <IconBtn
                    variant="ghost"
                    label={loopSelection ? 'Loop on' : 'Loop off'}
                    disabled={busy}
                    aria-pressed={loopSelection}
                    className={cn(
                      'size-8 [&_svg]:size-4',
                      loopSelection ? 'text-highlight hover:text-highlight' : undefined
                    )}
                    onClick={() => setLoopSelection((prev) => !prev)}
                  >
                    <Repeat strokeWidth={1.5} />
                  </IconBtn>
                </>
              )}
            </div>
            <div className="ml-auto flex items-center gap-2">
              <PublishBtn clip={clip} signedIn={signedIn} busy={busy} />
              {clip.localFile ? (
                <>
                  <ExportBtn
                    label={
                      discordExport?.id === clip.id
                        ? `Discord ${discordExport.percent}%`
                        : 'Export for Discord'
                    }
                    hint={
                      discordExport
                        ? 'Exporting for Discord…'
                        : 'Under 20 MB, with the trim and volumes above'
                    }
                    showHint
                    variant="secondary"
                    disabled={busy || discordExport !== null}
                    busy={discordExport?.id === clip.id}
                    onClick={runDiscordExport}
                  />
                  <ExportBtn
                    label="Save copy"
                    hint={exportHint}
                    variant="secondary"
                    disabled={!canExport}
                    busy={busy}
                    onClick={() => void runTrim('save-as-new')}
                  />
                  <ExportBtn
                    label="Overwrite"
                    hint={exportHint}
                    disabled={!canExport}
                    busy={busy}
                    onClick={() => setOverwriteOpen(true)}
                  />
                </>
              ) : null}
              {clip.localFile ? (
                <ClipMenu clip={clip} busy={busy} onDeleted={() => navigate('/library')} />
              ) : (
                <IconBtn
                  variant="destructive"
                  label="Delete"
                  disabled={busy}
                  className="size-8 [&_svg]:size-4"
                  onClick={() => {
                    if (!hasDesktopApi()) return
                    void ipc.deleteClip(clip.id).then((deleted) => {
                      if (deleted) navigate('/library')
                    })
                  }}
                >
                  <Trash2 strokeWidth={1.5} />
                </IconBtn>
              )}
            </div>
          </div>
        </div>

        {error ? (
          <div className="mx-auto w-full max-w-[96rem] px-8 pb-3">
            <Alert variant="destructive">
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          </div>
        ) : null}
      </section>

      <AlertDialog open={overwriteOpen} onOpenChange={setOverwriteOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Overwrite original file?</AlertDialogTitle>
            <AlertDialogDescription>
              This replaces the original clip on disk. The untrimmed file will be lost.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!canExport) return
                void runTrim('overwrite')
              }}
            >
              Overwrite
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </main>
  )
}

function TimeStat(props: {
  label: string
  value: string
  muted?: boolean
  accent?: boolean
  hint?: string
  shortcut?: string
  onClick?: () => void
}): React.JSX.Element {
  const body = (
    <>
      <div className="eyebrow">{props.label}</div>
      <div
        className={cn(
          'tc mt-1 text-[16px] leading-none font-semibold',
          props.accent
            ? 'text-highlight'
            : props.muted
              ? 'text-muted-foreground'
              : 'text-foreground'
        )}
      >
        {props.value}
      </div>
    </>
  )
  if (!props.onClick) {
    return <div className="min-w-0 px-3 py-2">{body}</div>
  }
  return (
    <Hint label={props.hint ?? props.label} shortcut={props.shortcut}>
      <button
        type="button"
        className="min-w-0 px-3 py-2 text-left transition-colors hover:bg-accent/60"
        onClick={props.onClick}
      >
        {body}
      </button>
    </Hint>
  )
}

function MetaChip(props: { children: ReactNode }): React.JSX.Element {
  return (
    <span className="tc flex h-6 items-center border border-border-strong px-2 text-[10.5px] text-muted-foreground">
      {props.children}
    </span>
  )
}

function EditorEmpty(props: {
  title: string
  body: string
  onBack: () => void
}): React.JSX.Element {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 pb-12 text-center">
      <span className="bug">
        <span>EDIT</span>
        <span>--:--</span>
      </span>
      <h2 className="display-title mt-2 text-[34px]">{props.title}</h2>
      <p className="text-[13.5px] text-muted-foreground">{props.body}</p>
      <Button size="lg" className="mt-2" onClick={props.onBack}>
        Browse clips
      </Button>
    </div>
  )
}

function IconBtn({
  label,
  shortcut,
  className,
  children,
  disabled,
  ...props
}: ComponentProps<typeof Button> & {
  label: string
  shortcut?: string
}): React.JSX.Element {
  return (
    <Hint label={label} shortcut={shortcut} disabled={disabled}>
      <Button
        type="button"
        size="icon"
        disabled={disabled}
        aria-label={label}
        className={className}
        {...props}
      >
        {children}
      </Button>
    </Hint>
  )
}

function ExportBtn(props: {
  label: string
  hint?: string
  variant?: ComponentProps<typeof Button>['variant']
  disabled?: boolean
  busy?: boolean
  /** Show the hint while enabled too, not only to explain why it is disabled. */
  showHint?: boolean
  onClick: () => void
}): React.JSX.Element {
  const button = (
    <Button
      type="button"
      variant={props.variant}
      size="lg"
      disabled={props.disabled}
      className="tabular-nums"
      onClick={props.onClick}
    >
      {props.busy ? <Loader2 className="animate-spin" strokeWidth={1.5} /> : null}
      {props.label}
    </Button>
  )
  if (!props.hint) return button
  if (!props.disabled) {
    return props.showHint ? <Hint label={props.hint}>{button}</Hint> : button
  }
  return (
    <Hint label={props.hint} disabled>
      {button}
    </Hint>
  )
}

function PublishBtn(props: {
  clip: LibraryClip
  signedIn: boolean
  busy: boolean
}): React.JSX.Element | null {
  const { clip, busy } = props
  if (clip.publicUrl) {
    return (
      <IconBtn
        variant="ghost"
        label="Copy link"
        disabled={busy}
        className="size-8 [&_svg]:size-4"
        onClick={() => {
          if (!hasDesktopApi()) return
          void ipc.copyClipLink(clip.id)
        }}
      >
        <Share strokeWidth={1.5} />
      </IconBtn>
    )
  }
  if (clip.cloudStatus === 'uploading') {
    return (
      <IconBtn variant="ghost" label="Uploading" disabled className="size-8 [&_svg]:size-4">
        <Loader2 className="animate-spin" strokeWidth={1.5} />
      </IconBtn>
    )
  }
  if (!canUpload(clip, props.signedIn)) return null
  return (
    <IconBtn
      variant="ghost"
      label={clip.cloudStatus === 'error' ? 'Retry upload' : 'Upload'}
      disabled={busy}
      className="size-8 [&_svg]:size-4"
      onClick={() => {
        if (!hasDesktopApi()) return
        void ipc.uploadClip(clip.id)
      }}
    >
      <Upload strokeWidth={1.5} />
    </IconBtn>
  )
}

function canUpload(clip: LibraryClip, signedIn: boolean): boolean {
  if (!signedIn || !clip.localFile) return false
  if (clip.cloudStatus === 'uploading') return false
  return !clip.cloudId || clip.cloudStatus === 'error' || clip.cloudStatus === 'local'
}

function ClipMenu(props: {
  clip: LibraryClip
  busy: boolean
  onDeleted: () => void
}): React.JSX.Element {
  const { clip, busy, onDeleted } = props
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              disabled={busy}
              aria-label="More"
              className="size-8 [&_svg]:size-4"
            >
              <MoreHorizontal strokeWidth={1.5} />
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>More</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end">
        {clip.localFile ? (
          <DropdownMenuItem
            disabled={busy}
            onClick={() => {
              if (!hasDesktopApi()) return
              void ipc.revealClip(clip.id)
            }}
          >
            <FolderOpen strokeWidth={1.5} />
            Show in folder
          </DropdownMenuItem>
        ) : null}
        {clip.localFile && clip.publicUrl ? (
          <DropdownMenuItem
            disabled={busy}
            onClick={() => {
              if (!hasDesktopApi()) return
              void ipc.deleteLocalFile(clip.id)
            }}
          >
            <FileX strokeWidth={1.5} />
            Delete local file
          </DropdownMenuItem>
        ) : null}
        {clip.publicUrl ? (
          <DropdownMenuItem
            disabled={busy}
            onClick={() => {
              if (!hasDesktopApi()) return
              void ipc.removeFromCloud(clip.id).then((removed) => {
                if (removed && !clip.localFile) onDeleted()
              })
            }}
          >
            Remove from cloud
          </DropdownMenuItem>
        ) : null}
        {clip.localFile || clip.publicUrl ? <DropdownMenuSeparator /> : null}
        <DropdownMenuItem
          variant="destructive"
          disabled={busy}
          onClick={() => {
            if (!hasDesktopApi()) return
            void ipc.deleteClip(clip.id).then((deleted) => {
              if (deleted) onDeleted()
            })
          }}
        >
          <Trash2 strokeWidth={1.5} />
          Delete
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function Hint(props: {
  label: string
  shortcut?: string
  disabled?: boolean
  children: ReactNode
}): React.JSX.Element {
  const trigger = props.disabled ? (
    <span className="inline-flex">{props.children}</span>
  ) : (
    props.children
  )
  return (
    <Tooltip>
      <TooltipTrigger asChild>{trigger}</TooltipTrigger>
      <TooltipContent>
        {props.label}
        {props.shortcut ? (
          <span className="ml-1.5 text-muted-foreground">{props.shortcut}</span>
        ) : null}
      </TooltipContent>
    </Tooltip>
  )
}

function applyClip(
  next: LibraryClip | null,
  setClip: (clip: LibraryClip | null) => void,
  setMissing: (value: boolean) => void,
  setDuration: (value: number) => void,
  setStart: (value: number) => void,
  setEnd: (value: number) => void,
  setCurrent: (value: number) => void,
  setPlaying: (value: boolean) => void,
  setError: (value: string | null) => void
): void {
  if (!next) {
    setClip(null)
    setMissing(true)
    return
  }
  setMissing(false)
  setClip(next)
  setError(null)
  const seconds = Math.max(0.25, next.durationMs / 1000)
  setDuration(seconds)
  setStart(0)
  setEnd(seconds)
  setCurrent(0)
  setPlaying(false)
}
