import type { ComponentProps } from 'react'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { DEFAULT_HOTKEY, formatHotkey } from '@echoframe/shared'
import { Ellipsis, Loader2, Play, Search, Share, Upload } from 'lucide-react'
import { toast } from 'sonner'
import { AppLogo } from '@/components/AppLogo'
import { ClipTitle } from '@/components/ClipTitle'
import { FolderNameDialog } from '@/components/FolderNameDialog'
import { CLIP_DRAG_TYPE, FolderRail, type FolderView } from '@/components/FolderRail'
import { PageHeader } from '@/components/PageHeader'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Kbd } from '@/components/ui/kbd'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { useCaptureStatus } from '@/lib/capture'
import { formatBytes, formatClipDate, formatDuration } from '@/lib/formats'
import { hasDesktopApi, ipc, ipcErrorMessage } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import type { ClipFolder, LibraryClip } from '../../../tauri/api'

const filters = [
  { value: 'all', label: 'All' },
  { value: 'local', label: 'Local' },
  { value: 'cloud', label: 'Uploaded' }
] as const

/** Remembers the open folder across the editor and restarts. */
const VIEW_KEY = 'rikette.library.folder'

/** What the name dialog is doing: making a folder (and maybe filing clips into it) or renaming one. */
type FolderDialog = { kind: 'create'; clipIds: string[] } | { kind: 'rename'; folder: ClipFolder }

export function Library(): React.JSX.Element {
  const navigate = useNavigate()
  const [hotkey, setHotkey] = useState(DEFAULT_HOTKEY)
  const [capture] = useCaptureStatus()
  const [clips, setClips] = useState<LibraryClip[]>([])
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<'all' | 'local' | 'cloud'>('all')
  const [signedIn, setSignedIn] = useState(false)
  const [folders, setFolders] = useState<ClipFolder[]>([])
  const [view, setViewState] = useState<FolderView>(() => localStorage.getItem(VIEW_KEY) ?? 'all')
  const [dragging, setDragging] = useState(false)
  const [folderDialog, setFolderDialog] = useState<FolderDialog | null>(null)

  const setView = (next: FolderView): void => {
    setViewState(next)
    localStorage.setItem(VIEW_KEY, next)
  }

  useEffect(() => {
    if (!hasDesktopApi()) return
    const reload = (): void => {
      void ipc.listClips().then(setClips)
      void ipc.listFolders().then(setFolders)
    }
    void ipc.getSettings().then((settings) => setHotkey(settings.hotkey))
    void ipc.getMe().then((me) => setSignedIn(Boolean(me)))
    reload()
    const offClips = ipc.onClipsChanged(reload)
    const offAuth = ipc.onAuthChanged((me) => setSignedIn(Boolean(me)))
    return () => {
      offClips()
      offAuth()
    }
  }, [])

  const folderById = useMemo(() => new Map(folders.map((folder) => [folder.id, folder])), [folders])
  // A deleted folder falls back to everything.
  const activeView = view === 'all' || view === 'unsorted' || folderById.has(view) ? view : 'all'
  const activeFolder = folderById.get(activeView) ?? null

  const counts = useMemo(() => {
    const byFolder = new Map<string, number>()
    let unsorted = 0
    for (const clip of clips) {
      if (clip.folderId && folderById.has(clip.folderId)) {
        byFolder.set(clip.folderId, (byFolder.get(clip.folderId) ?? 0) + 1)
      } else {
        unsorted += 1
      }
    }
    return { all: clips.length, unsorted, byFolder }
  }, [clips, folderById])

  const inView = useMemo(
    () =>
      clips.filter((clip) => {
        if (activeView === 'all') return true
        const folderId = clip.folderId && folderById.has(clip.folderId) ? clip.folderId : null
        if (activeView === 'unsorted') return folderId === null
        return folderId === activeView
      }),
    [clips, activeView, folderById]
  )

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return inView
      .filter((clip) => {
        if (filter === 'local' && clip.cloudStatus !== 'local') return false
        if (filter === 'cloud' && clip.cloudStatus === 'local') return false
        if (!needle) return true
        const hay = `${clip.title} ${clip.gameTitle ?? ''}`.toLowerCase()
        return hay.includes(needle)
      })
      .sort((a, b) => b.createdAt - a.createdAt)
  }, [inView, query, filter])

  const totalMs = inView.reduce((sum, clip) => sum + clip.durationMs, 0)

  const moveClips = async (ids: string[], folderId: string | null): Promise<void> => {
    if (!hasDesktopApi()) return
    const moving = clips.filter((clip) => ids.includes(clip.id) && clip.folderId !== folderId)
    if (moving.length === 0) return
    // Update the grid right away; the main process confirms with clips:changed.
    setClips((prev) => prev.map((clip) => (ids.includes(clip.id) ? { ...clip, folderId } : clip)))
    try {
      await ipc.moveClips(ids, folderId)
    } catch (error) {
      toast.error(ipcErrorMessage(error))
      void ipc.listClips().then(setClips)
      return
    }
    const where = folderId ? (folderById.get(folderId)?.name ?? 'folder') : 'Unsorted'
    const what = moving.length === 1 ? `"${moving[0].title}"` : `${moving.length} clips`
    toast.success(`Moved ${what} to ${where}`, {
      action: {
        label: 'Undo',
        onClick: () => {
          for (const clip of moving) void ipc.moveClips([clip.id], clip.folderId)
        }
      }
    })
  }

  const submitFolderName = async (name: string): Promise<void> => {
    if (!folderDialog || !hasDesktopApi()) return
    if (folderDialog.kind === 'rename') {
      const renamed = await ipc.renameFolder(folderDialog.folder.id, name)
      setFolders((prev) => prev.map((item) => (item.id === renamed.id ? renamed : item)))
      return
    }
    const folder = await ipc.createFolder(name)
    setFolders((prev) => [...prev, folder])
    if (folderDialog.clipIds.length > 0) {
      await ipc.moveClips(folderDialog.clipIds, folder.id)
      toast.success(
        folderDialog.clipIds.length === 1
          ? `Made "${folder.name}" and filed the clip there`
          : `Made "${folder.name}" and filed ${folderDialog.clipIds.length} clips there`
      )
    } else {
      setView(folder.id)
    }
  }

  const showRail = clips.length > 0 || folders.length > 0

  return (
    <main className="flex h-full flex-col">
      <PageHeader
        kicker={
          inView.length > 0
            ? `${inView.length} clip${inView.length === 1 ? '' : 's'} · ${formatDuration(totalMs)} of highlights`
            : activeFolder
              ? 'Folder'
              : 'Your highlight reel'
        }
        title={activeFolder?.name ?? (activeView === 'unsorted' ? 'Unsorted' : 'Clips')}
      >
        <div className="ml-auto flex items-center gap-3">
          {clips.length > 0 ? (
            <>
              <ToggleGroup
                type="single"
                value={filter}
                onValueChange={(value) => {
                  if (value === 'all' || value === 'local' || value === 'cloud') setFilter(value)
                }}
              >
                {filters.map((item) => (
                  <ToggleGroupItem key={item.value} value={item.value}>
                    {item.label}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
              <label className="relative">
                <Search
                  className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                  strokeWidth={2}
                />
                <Input
                  className="w-56 pl-8"
                  placeholder="Search clips or games"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  spellCheck={false}
                />
              </label>
            </>
          ) : null}
          <Button
            type="button"
            size="lg"
            disabled={!capture?.ready || capture.saving}
            onClick={() => {
              if (!hasDesktopApi()) return
              void ipc.saveClip()
            }}
          >
            {capture?.saving ? <Loader2 className="animate-spin" /> : null}
            {capture?.saving ? 'Saving…' : 'Clip it'}
          </Button>
        </div>
      </PageHeader>

      <div className="flex min-h-0 flex-1">
        {showRail ? (
          <FolderRail
            folders={folders}
            counts={counts}
            view={activeView}
            dragging={dragging}
            onView={setView}
            onDropClip={(clipId, folderId) => void moveClips([clipId], folderId)}
            onNewFolder={(clipId) =>
              setFolderDialog({ kind: 'create', clipIds: clipId ? [clipId] : [] })
            }
            onRename={(folder) => setFolderDialog({ kind: 'rename', folder })}
            onDelete={(folder) => {
              if (!hasDesktopApi()) return
              void ipc.deleteFolder(folder.id)
            }}
          />
        ) : null}
        {filtered.length === 0 ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-4 px-6 pb-10 text-center">
            <span className="bug">
              <span>REPLAY</span>
              <span>--:--</span>
            </span>
            <h2 className="display-title text-[30px]">
              {clips.length === 0
                ? 'No clips yet'
                : inView.length === 0
                  ? activeFolder
                    ? 'Empty folder'
                    : 'All sorted'
                  : 'Nothing found'}
            </h2>
            <p className="max-w-sm text-[13.5px] text-muted-foreground">
              {clips.length === 0 ? (
                <>
                  Hit{' '}
                  <Kbd className="mx-1 h-6 px-2">
                    {formatHotkey(hotkey, hasDesktopApi() ? ipc.platform : 'win32')}
                  </Kbd>{' '}
                  in a game and it lands here.
                </>
              ) : inView.length === 0 ? (
                activeFolder ? (
                  <>
                    Drag clips from <strong className="text-foreground">All clips</strong> onto{' '}
                    <strong className="text-foreground">{activeFolder.name}</strong> in the list on
                    the left, or use a clip&apos;s <strong className="text-foreground">⋯</strong>{' '}
                    menu → Move to folder.
                  </>
                ) : (
                  'Every clip is in a folder.'
                )
              ) : (
                'No clips match that search.'
              )}
            </p>
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto px-8 pt-1 pb-10">
            <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-x-4 gap-y-5">
              {filtered.map((clip, index) => (
                <ClipCard
                  key={clip.id}
                  index={index}
                  clip={clip}
                  signedIn={signedIn}
                  folders={folders}
                  folderName={
                    activeView === 'all' && clip.folderId
                      ? (folderById.get(clip.folderId)?.name ?? null)
                      : null
                  }
                  onDragging={setDragging}
                  onMove={(folderId) => void moveClips([clip.id], folderId)}
                  onNewFolder={() => setFolderDialog({ kind: 'create', clipIds: [clip.id] })}
                  onOpen={() => navigate(`/editor/${clip.id}`)}
                  onRenamed={(title) => {
                    setClips((prev) =>
                      prev.map((item) => (item.id === clip.id ? { ...item, title } : item))
                    )
                  }}
                />
              ))}
            </div>
          </div>
        )}
      </div>

      <FolderNameDialog
        open={folderDialog !== null}
        onOpenChange={(open) => {
          if (!open) setFolderDialog(null)
        }}
        title={folderDialog?.kind === 'rename' ? 'Rename folder' : 'New folder'}
        description={
          folderDialog?.kind === 'rename'
            ? 'Only the name in Rikette changes. Your video files stay where they are.'
            : folderDialog?.kind === 'create' && folderDialog.clipIds.length > 0
              ? 'The clip goes straight in. Folders only sort your library; files stay where they are on disk.'
              : 'Folders sort your library. Your video files stay where they are on disk.'
        }
        confirmLabel={folderDialog?.kind === 'rename' ? 'Rename' : 'Create folder'}
        initialName={folderDialog?.kind === 'rename' ? folderDialog.folder.name : ''}
        onSubmit={submitFolderName}
      />
    </main>
  )
}

function ClipCard(props: {
  index: number
  clip: LibraryClip
  signedIn: boolean
  folders: ClipFolder[]
  folderName: string | null
  onDragging: (dragging: boolean) => void
  onMove: (folderId: string | null) => void
  onNewFolder: () => void
  onOpen: () => void
  onRenamed: (title: string) => void
}): React.JSX.Element {
  const { clip } = props
  const status = clipStatus(clip)
  const upload = canUpload(clip, props.signedIn)
  const [dragged, setDragged] = useState(false)

  return (
    <article
      className={cn('rise-in group/card', dragged && 'opacity-40')}
      style={{ '--i': Math.min(props.index, 16) } as React.CSSProperties}
      draggable
      onDragStart={(event) => {
        // Text selected while renaming drags as text, not as the clip.
        if ((event.target as HTMLElement).closest('input')) return
        event.dataTransfer.setData(CLIP_DRAG_TYPE, clip.id)
        event.dataTransfer.effectAllowed = 'move'
        setDragged(true)
        props.onDragging(true)
      }}
      onDragEnd={() => {
        setDragged(false)
        props.onDragging(false)
      }}
      onContextMenu={(event) => {
        event.preventDefault()
        if (!hasDesktopApi()) return
        void ipc.clipMenu(clip.id)
      }}
    >
      <div className="relative">
        <button
          type="button"
          aria-label={`Edit ${clip.title}`}
          onClick={props.onOpen}
          className="relative block aspect-video w-full overflow-hidden rounded-sm bg-black outline-1 outline-border transition-[outline-color] duration-150 group-hover/card:outline-2 group-hover/card:outline-primary"
        >
          {clip.thumbUrl ? (
            <img
              src={clip.thumbUrl}
              alt=""
              loading="lazy"
              decoding="async"
              draggable={false}
              className="size-full object-cover transition-transform duration-500 ease-out group-hover/card:scale-[1.04]"
            />
          ) : (
            <span className="grid size-full place-items-center">
              <AppLogo className="size-10 opacity-30" />
            </span>
          )}
          <span className="absolute inset-0 bg-[linear-gradient(0deg,var(--scrim),transparent_45%)]" />
          <span className="absolute inset-0 grid place-items-center opacity-0 transition-opacity duration-150 group-hover/card:opacity-100">
            <span className="grid size-11 place-items-center bg-primary text-primary-foreground">
              <Play className="size-5 translate-x-px fill-primary-foreground text-primary-foreground" />
            </span>
          </span>
          <span className="absolute top-2 left-2">
            <Badge variant={status.variant}>{status.label}</Badge>
          </span>
          <span className="tc absolute right-2 bottom-2 bg-[var(--scrim)] px-1.5 py-0.5 text-[10.5px] text-foreground">
            {formatDuration(clip.durationMs)}
          </span>
          {clip.gameTitle?.trim() ? (
            <span className="absolute bottom-2 left-2 max-w-[60%] truncate font-display text-[13px] font-bold text-foreground">
              {clip.gameTitle.trim()}
            </span>
          ) : null}
        </button>

        <div
          className="absolute top-1.5 right-1.5 flex gap-1 opacity-0 transition-opacity duration-200 group-focus-within/card:opacity-100 group-hover/card:opacity-100"
          onClick={(event) => event.stopPropagation()}
        >
          {clip.publicUrl ? (
            <IconAction
              label="Copy link"
              onClick={() => {
                if (!hasDesktopApi()) return
                void ipc.copyClipLink(clip.id)
              }}
            >
              <Share strokeWidth={2} />
            </IconAction>
          ) : clip.cloudStatus === 'uploading' ? (
            <IconAction label="Uploading" disabled>
              <Loader2 className="animate-spin" strokeWidth={2} />
            </IconAction>
          ) : upload ? (
            <IconAction
              label={clip.cloudStatus === 'error' ? 'Retry upload' : 'Upload'}
              onClick={() => {
                if (!hasDesktopApi()) return
                void ipc.uploadClip(clip.id)
              }}
            >
              <Upload strokeWidth={2} />
            </IconAction>
          ) : null}
          <ClipMenu
            clip={clip}
            folders={props.folders}
            onMove={props.onMove}
            onNewFolder={props.onNewFolder}
          />
        </div>
      </div>

      <div className="mt-2">
        <ClipTitle id={clip.id} title={clip.title} renameWith="icon" onRenamed={props.onRenamed} />
        <p className="tc mt-0.5 truncate text-[10px] text-muted-foreground">
          {formatClipDate(clip.createdAt)}
          {clip.localFile ? ` · ${formatBytes(clip.bytes)}` : ''}
          {props.folderName ? ` · ${props.folderName}` : ''}
        </p>
      </div>
    </article>
  )
}

function ClipMenu(props: {
  clip: LibraryClip
  folders: ClipFolder[]
  onMove: (folderId: string | null) => void
  onNewFolder: () => void
}): React.JSX.Element {
  const { clip } = props
  const filed = props.folders.some((folder) => folder.id === clip.folderId)
  return (
    <DropdownMenu>
      <Tooltip>
        <TooltipTrigger asChild>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label="More"
              className={chipButton}
            >
              <Ellipsis strokeWidth={2} />
            </Button>
          </DropdownMenuTrigger>
        </TooltipTrigger>
        <TooltipContent>More</TooltipContent>
      </Tooltip>
      <DropdownMenuContent align="end" onClick={(event) => event.stopPropagation()}>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>Move to folder</DropdownMenuSubTrigger>
          <DropdownMenuPortal>
            <DropdownMenuSubContent className="max-h-80 overflow-y-auto">
              {props.folders.map((folder) => (
                <DropdownMenuCheckboxItem
                  key={folder.id}
                  checked={clip.folderId === folder.id}
                  onSelect={() => props.onMove(folder.id)}
                >
                  <span className="max-w-48 truncate">{folder.name}</span>
                </DropdownMenuCheckboxItem>
              ))}
              {props.folders.length > 0 ? (
                <DropdownMenuCheckboxItem checked={!filed} onSelect={() => props.onMove(null)}>
                  Unsorted
                </DropdownMenuCheckboxItem>
              ) : null}
              {props.folders.length > 0 ? <DropdownMenuSeparator /> : null}
              <DropdownMenuItem onSelect={props.onNewFolder}>New folder…</DropdownMenuItem>
            </DropdownMenuSubContent>
          </DropdownMenuPortal>
        </DropdownMenuSub>
        <DropdownMenuSeparator />
        {clip.localFile ? (
          <DropdownMenuItem
            onSelect={() => {
              if (!hasDesktopApi()) return
              // The main process reports progress and failures with toasts.
              void ipc.exportForDiscord({ id: clip.id }).catch(() => undefined)
            }}
          >
            Export for Discord
          </DropdownMenuItem>
        ) : null}
        {clip.localFile ? (
          <DropdownMenuItem
            onSelect={() => {
              if (!hasDesktopApi()) return
              void ipc.revealClip(clip.id)
            }}
          >
            Show in folder
          </DropdownMenuItem>
        ) : null}
        {clip.localFile && clip.publicUrl ? (
          <DropdownMenuItem
            onSelect={() => {
              if (!hasDesktopApi()) return
              void ipc.deleteLocalFile(clip.id)
            }}
          >
            Delete local file
          </DropdownMenuItem>
        ) : null}
        {clip.publicUrl ? (
          <DropdownMenuItem
            onSelect={() => {
              if (!hasDesktopApi()) return
              void ipc.removeFromCloud(clip.id)
            }}
          >
            Remove from cloud
          </DropdownMenuItem>
        ) : null}
        {clip.localFile || clip.publicUrl ? <DropdownMenuSeparator /> : null}
        <DropdownMenuItem
          variant="destructive"
          onSelect={() => {
            if (!hasDesktopApi()) return
            void ipc.deleteClip(clip.id)
          }}
        >
          Delete
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/* Small dark chips that sit on the thumbnail. */
const chipButton =
  'size-7 rounded-sm bg-[var(--scrim)] text-foreground hover:bg-primary hover:text-primary-foreground [&_svg]:size-3.5'

function IconAction({
  label,
  children,
  disabled,
  className,
  ...props
}: ComponentProps<typeof Button> & { label: string }): React.JSX.Element {
  const button = (
    <Button
      type="button"
      size="icon"
      variant="ghost"
      aria-label={label}
      disabled={disabled}
      className={cn(chipButton, className)}
      {...props}
    >
      {children}
    </Button>
  )
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        {disabled ? <span className="inline-flex">{button}</span> : button}
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

function canUpload(clip: LibraryClip, signedIn: boolean): boolean {
  if (!signedIn || !clip.localFile) return false
  if (clip.cloudStatus === 'uploading') return false
  return !clip.cloudId || clip.cloudStatus === 'error' || clip.cloudStatus === 'local'
}

function clipStatus(clip: LibraryClip): {
  label: string
  variant: 'default' | 'secondary' | 'outline' | 'destructive'
} {
  if (clip.cloudStatus === 'uploading') return { label: 'Uploading', variant: 'default' }
  if (clip.cloudStatus === 'error') return { label: 'Upload failed', variant: 'destructive' }
  if (clip.publicUrl && !clip.localFile) return { label: 'Online', variant: 'secondary' }
  if (clip.publicUrl) return { label: 'Uploaded', variant: 'secondary' }
  return { label: 'Local', variant: 'outline' }
}
