import { useState, type DragEvent, type ReactNode } from 'react'
import { Ellipsis, Folder, FolderPlus, Inbox, Layers } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import type { ClipFolder } from '../../../tauri/api'

/** `all`, `unsorted`, or a folder id. */
export type FolderView = string

/** Set on a clip card's drag so folder rows know what they are receiving. */
export const CLIP_DRAG_TYPE = 'application/x-rikette-clip'

export function FolderRail(props: {
  folders: ClipFolder[]
  counts: { all: number; unsorted: number; byFolder: Map<string, number> }
  view: FolderView
  dragging: boolean
  onView: (view: FolderView) => void
  /** `folderId` null = Unsorted. */
  onDropClip: (clipId: string, folderId: string | null) => void
  /** With a clip id when a clip was dropped on "New folder". */
  onNewFolder: (clipId?: string) => void
  onRename: (folder: ClipFolder) => void
  onDelete: (folder: ClipFolder) => void
}): React.JSX.Element {
  const hasFolders = props.folders.length > 0

  return (
    <aside className="flex w-52 shrink-0 flex-col border-r border-border">
      <div className="flex h-8 items-center px-3">
        <span className={cn('eyebrow', props.dragging && 'text-highlight')}>
          {props.dragging ? 'Drop on a folder' : 'Folders'}
        </span>
      </div>
      <nav className="min-h-0 flex-1 overflow-y-auto border-t border-border">
        <RailRow
          icon={<Layers />}
          label="All clips"
          count={props.counts.all}
          active={props.view === 'all'}
          onClick={() => props.onView('all')}
        />
        {hasFolders ? (
          <RailRow
            icon={<Inbox />}
            label="Unsorted"
            count={props.counts.unsorted}
            active={props.view === 'unsorted'}
            onClick={() => props.onView('unsorted')}
            onDropClip={(clipId) => props.onDropClip(clipId, null)}
          />
        ) : null}
        {props.folders.map((folder) => (
          <RailRow
            key={folder.id}
            icon={<Folder />}
            label={folder.name}
            count={props.counts.byFolder.get(folder.id) ?? 0}
            active={props.view === folder.id}
            onClick={() => props.onView(folder.id)}
            onDoubleClick={() => props.onRename(folder)}
            onDropClip={(clipId) => props.onDropClip(clipId, folder.id)}
            menu={{
              onRename: () => props.onRename(folder),
              onDelete: () => props.onDelete(folder)
            }}
          />
        ))}
        <RailRow
          icon={<FolderPlus />}
          label="New folder"
          muted
          onClick={() => props.onNewFolder()}
          onDropClip={(clipId) => props.onNewFolder(clipId)}
        />
        {!hasFolders ? (
          <p className="px-3 pt-3 text-[12px] leading-[1.45] text-muted-foreground">
            Sort clips by game, squad or moment. Make a folder, then drag clips onto it.
          </p>
        ) : null}
      </nav>
    </aside>
  )
}

function RailRow(props: {
  icon: ReactNode
  label: string
  count?: number
  active?: boolean
  muted?: boolean
  onClick: () => void
  onDoubleClick?: () => void
  onDropClip?: (clipId: string) => void
  menu?: { onRename: () => void; onDelete: () => void }
}): React.JSX.Element {
  const [over, setOver] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const droppable = Boolean(props.onDropClip)

  const accepts = (event: DragEvent): boolean =>
    droppable && event.dataTransfer.types.includes(CLIP_DRAG_TYPE)

  return (
    <div
      className="group/row relative"
      onDragEnter={(event) => {
        if (!accepts(event)) return
        event.preventDefault()
        setOver(true)
      }}
      onDragOver={(event) => {
        if (!accepts(event)) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
        if (!over) setOver(true)
      }}
      onDragLeave={(event) => {
        if (event.currentTarget.contains(event.relatedTarget as Node | null)) return
        setOver(false)
      }}
      onDrop={(event) => {
        if (!accepts(event)) return
        event.preventDefault()
        setOver(false)
        const clipId = event.dataTransfer.getData(CLIP_DRAG_TYPE)
        if (clipId) props.onDropClip?.(clipId)
      }}
      onContextMenu={(event) => {
        if (!props.menu) return
        event.preventDefault()
        setMenuOpen(true)
      }}
    >
      <button
        type="button"
        onClick={props.onClick}
        onDoubleClick={props.onDoubleClick}
        title={props.onDoubleClick ? 'Double-click to rename' : undefined}
        className={cn(
          'flex h-10 w-full items-center gap-2.5 border-b border-border px-3 text-left transition-colors duration-150 [&_svg]:size-3.5 [&_svg]:shrink-0',
          over
            ? 'bg-primary text-primary-foreground'
            : props.active
              ? 'bg-card text-foreground shadow-[inset_3px_0_0_var(--primary)]'
              : 'text-muted-foreground hover:bg-accent hover:text-foreground',
          props.muted && !over && 'font-normal'
        )}
      >
        <span className={cn(props.active && !over && 'text-primary')}>{props.icon}</span>
        <span
          className={cn(
            'min-w-0 flex-1 truncate',
            props.muted ? 'text-[13px]' : 'font-display text-[15px] font-bold'
          )}
        >
          {props.label}
        </span>
        {props.count !== undefined ? (
          <span
            className={cn(
              'tc text-[10px] transition-opacity',
              props.menu && 'group-focus-within/row:opacity-0 group-hover/row:opacity-0',
              menuOpen && 'opacity-0'
            )}
          >
            {props.count}
          </span>
        ) : null}
      </button>
      {props.menu ? (
        <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`${props.label} options`}
              className={cn(
                'absolute top-1/2 right-1.5 size-7 -translate-y-1/2 opacity-0 group-focus-within/row:opacity-100 group-hover/row:opacity-100 [&_svg]:size-3.5',
                menuOpen && 'opacity-100',
                over && 'hidden'
              )}
            >
              <Ellipsis strokeWidth={2} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" side="right">
            <DropdownMenuItem onSelect={props.menu.onRename}>Rename</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive" onSelect={props.menu.onDelete}>
              Delete folder
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
    </div>
  )
}
