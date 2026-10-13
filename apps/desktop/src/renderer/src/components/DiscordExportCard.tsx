import { useEffect, useState } from 'react'
import { FolderOpen, GripVertical, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { formatBytes } from '@/lib/formats'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import type { DiscordExportReady } from '../../../tauri/api'

type CardState = {
  id: string
  title: string
  percent: number | null
  ready: DiscordExportReady | null
}

/** Follows the running Discord export on every page, then becomes a handle that drags the
 * file straight into Discord. */
export function DiscordExportCard(): React.JSX.Element | null {
  const [state, setState] = useState<CardState | null>(null)

  useEffect(() => {
    if (!hasDesktopApi()) return
    const offProgress = ipc.onExportProgress(({ id, title, percent }) => {
      // A finished export keeps its card; a failed one (no ready event) clears it.
      setState((prev) =>
        percent === null ? (prev?.ready ? prev : null) : { id, title, percent, ready: null }
      )
    })
    const offReady = ipc.onDiscordReady((ready) => {
      setState({ id: ready.id, title: ready.title, percent: null, ready })
    })
    return () => {
      offProgress()
      offReady()
    }
  }, [])

  if (!state) return null
  const { ready } = state

  return (
    <div className="no-drag card animate-in fade-in slide-in-from-bottom-6 fixed bottom-10 left-6 z-50 flex w-[22rem] items-center gap-1 border-l-2 border-l-primary bg-popover p-2 shadow-[0_16px_32px_var(--shadow)] duration-300">
      {ready ? (
        <>
          <Tooltip>
            <TooltipTrigger asChild>
              <div
                draggable
                className="flex min-w-0 flex-1 cursor-grab items-center gap-2.5 rounded-sm p-1 transition-colors hover:bg-accent active:cursor-grabbing"
                onDragStart={(event) => {
                  event.preventDefault()
                  ipc.startDiscordDrag(ready.id)
                }}
              >
                <GripVertical
                  className="size-4 shrink-0 text-muted-foreground"
                  strokeWidth={1.75}
                />
                {ready.thumbUrl ? (
                  <img
                    src={ready.thumbUrl}
                    alt=""
                    draggable={false}
                    className="h-10 w-[4.5rem] shrink-0 object-cover"
                  />
                ) : null}
                <div className="min-w-0">
                  <p className="font-display text-[15px] font-bold text-foreground">
                    Drag into Discord
                  </p>
                  <p className="tc truncate text-[10px] text-muted-foreground">
                    {ready.title} · {formatBytes(ready.bytes)}
                  </p>
                </div>
              </div>
            </TooltipTrigger>
            <TooltipContent>Drop it in any Discord chat</TooltipContent>
          </Tooltip>
          <CardIcon
            label="Show in folder"
            onClick={() => void ipc.revealDiscordExport(ready.id).catch(() => setState(null))}
          >
            <FolderOpen strokeWidth={1.5} />
          </CardIcon>
          <CardIcon label="Dismiss" onClick={() => setState(null)}>
            <X strokeWidth={1.5} />
          </CardIcon>
        </>
      ) : (
        <div className="min-w-0 flex-1 p-1">
          <div className="flex items-baseline gap-2">
            <p className="text-[13.5px] font-semibold text-foreground">Exporting for Discord</p>
            <span className="ml-auto text-[12px] font-medium text-muted-foreground tabular-nums">
              {state.percent ?? 0}%
            </span>
          </div>
          <p className="mb-2 truncate text-[11px] text-muted-foreground">{state.title}</p>
          <Progress value={state.percent ?? 0} />
        </div>
      )}
    </div>
  )
}

function CardIcon(props: {
  label: string
  onClick: () => void
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={props.label}
          className="size-8 shrink-0 [&_svg]:size-4"
          onClick={props.onClick}
        >
          {props.children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{props.label}</TooltipContent>
    </Tooltip>
  )
}
