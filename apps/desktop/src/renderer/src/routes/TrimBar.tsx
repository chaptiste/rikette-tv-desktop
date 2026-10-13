import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

type DragKind = 'start' | 'end' | 'playhead'

export function TrimBar(props: {
  duration: number
  start: number
  end: number
  current: number
  disabled?: boolean
  onStart: (value: number) => void
  onEnd: (value: number) => void
  onSeek: (value: number) => void
}): React.JSX.Element {
  const trackRef = useRef<HTMLDivElement>(null)
  const drag = useRef<DragKind | null>(null)
  const [hover, setHover] = useState(false)

  const duration = Math.max(props.duration, 0.001)
  const startPct = (props.start / duration) * 100
  const endPct = (props.end / duration) * 100
  const playPct = (props.current / duration) * 100
  const minGap = Math.min(0.25, duration)

  const posFromEvent = useCallback(
    (clientX: number): number => {
      const track = trackRef.current
      if (!track) return 0
      const rect = track.getBoundingClientRect()
      const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width))
      return ratio * duration
    },
    [duration]
  )

  const applyDrag = useCallback(
    (kind: DragKind, time: number) => {
      if (kind === 'start') {
        props.onStart(Math.min(time, props.end - minGap))
        return
      }
      if (kind === 'end') {
        props.onEnd(Math.max(time, props.start + minGap))
        return
      }
      props.onSeek(Math.min(Math.max(time, 0), duration))
    },
    [duration, minGap, props]
  )

  useEffect(() => {
    const onMove = (event: PointerEvent): void => {
      if (!drag.current) return
      applyDrag(drag.current, posFromEvent(event.clientX))
    }
    const onUp = (): void => {
      drag.current = null
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    return () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
  }, [applyDrag, posFromEvent])

  const begin = (kind: DragKind, event: React.PointerEvent): void => {
    if (props.disabled) return
    event.preventDefault()
    event.stopPropagation()
    drag.current = kind
    applyDrag(kind, posFromEvent(event.clientX))
  }

  return (
    <div
      ref={trackRef}
      className={cn(
        'relative h-10 cursor-default select-none rounded-sm bg-background ring-1 ring-border',
        props.disabled && 'pointer-events-none opacity-60'
      )}
      onPointerDown={(event) => begin('playhead', event)}
      onPointerEnter={() => setHover(true)}
      onPointerLeave={() => setHover(false)}
    >
      <div
        className="absolute inset-y-0 border-y-2 border-primary bg-primary/25"
        style={{ left: `${startPct}%`, width: `${Math.max(0, endPct - startPct)}%` }}
      />
      <div
        className={cn(
          'pointer-events-none absolute -top-1.5 -bottom-1.5 z-20 w-0.5 -translate-x-1/2 bg-rec transition-opacity',
          hover ? 'opacity-100' : 'opacity-85'
        )}
        style={{ left: `${playPct}%` }}
      >
        <span className="absolute -top-1 left-1/2 h-1.5 w-2.5 -translate-x-1/2 bg-rec" />
      </div>
      <Handle side="start" pct={startPct} onPointerDown={(event) => begin('start', event)} />
      <Handle side="end" pct={endPct} onPointerDown={(event) => begin('end', event)} />
    </div>
  )
}

function Handle(props: {
  side: 'start' | 'end'
  pct: number
  onPointerDown: (event: React.PointerEvent) => void
}): React.JSX.Element {
  return (
    <button
      type="button"
      aria-label={props.side === 'start' ? 'Trim start' : 'Trim end'}
      className={cn(
        'absolute inset-y-0 z-10 flex w-3 cursor-ew-resize items-center justify-center bg-primary transition-[width,filter] hover:w-3.5 hover:brightness-110',
        props.side === 'start' ? '-translate-x-full rounded-l-sm' : 'rounded-r-sm'
      )}
      style={{ left: `${props.pct}%` }}
      onPointerDown={props.onPointerDown}
    >
      <span className="h-3.5 w-0.5 bg-primary-foreground/80" />
    </button>
  )
}
