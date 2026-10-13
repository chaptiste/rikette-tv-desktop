import { useEffect, useRef, useState, type MouseEvent } from 'react'
import { Pencil } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { cn } from '@/lib/utils'

export function ClipTitle(props: {
  id: string
  title: string
  className?: string
  renameWith?: 'click' | 'icon'
  showRenameIcon?: 'hover' | 'always'
  onRenamed?: (title: string) => void
}): React.JSX.Element {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(props.title)
  const inputRef = useRef<HTMLInputElement>(null)
  const skipCommit = useRef(false)
  const busy = useRef(false)
  const renameWith = props.renameWith ?? 'click'
  const showRenameIcon = props.showRenameIcon ?? 'hover'

  useEffect(() => {
    if (!editing) return
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [editing])

  const commit = async (): Promise<void> => {
    if (skipCommit.current) {
      skipCommit.current = false
      return
    }
    if (busy.current) return
    const next = draft.replace(/\s+/g, ' ').trim()
    setEditing(false)
    if (!next || next === props.title || !hasDesktopApi()) {
      setDraft(props.title)
      return
    }
    busy.current = true
    try {
      const clip = await ipc.renameClip(props.id, next)
      props.onRenamed?.(clip.title)
    } catch {
      setDraft(props.title)
    } finally {
      busy.current = false
    }
  }

  const startEditing = (event: MouseEvent): void => {
    event.preventDefault()
    event.stopPropagation()
    setDraft(props.title)
    setEditing(true)
  }

  if (editing) {
    return (
      <Input
        ref={inputRef}
        className={cn('h-7 px-1', props.className)}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => void commit()}
        onClick={(event) => event.stopPropagation()}
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          event.stopPropagation()
          if (event.key === 'Enter') {
            event.preventDefault()
            void commit()
          } else if (event.key === 'Escape') {
            event.preventDefault()
            skipCommit.current = true
            setDraft(props.title)
            setEditing(false)
          }
        }}
        spellCheck={false}
      />
    )
  }

  if (renameWith === 'icon') {
    return (
      <div className={cn('group/title flex h-7 min-w-0 items-center gap-0.5', props.className)}>
        <span className="min-w-0 truncate font-display text-[15px] font-bold leading-none">
          {props.title}
        </span>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              aria-label="Rename"
              className={cn(
                'size-7 shrink-0',
                showRenameIcon === 'hover' &&
                  'opacity-0 group-hover/title:opacity-100 group-focus-within/title:opacity-100'
              )}
              onClick={startEditing}
              onMouseDown={(event) => event.stopPropagation()}
            >
              <Pencil strokeWidth={1.5} />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Rename</TooltipContent>
        </Tooltip>
      </div>
    )
  }

  return (
    <button
      type="button"
      title="Rename"
      className={cn(
        'block w-full truncate rounded-sm px-0.5 text-left font-display text-[15px] font-bold outline-none hover:bg-accent focus-visible:bg-accent',
        props.className
      )}
      onClick={startEditing}
      onMouseDown={(event) => event.stopPropagation()}
    >
      {props.title}
    </button>
  )
}
