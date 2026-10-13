import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

/* A small mono key cap, the same one the website draws for Alt + 1. */
function Kbd({ className, ...props }: ComponentProps<'kbd'>): React.JSX.Element {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "kbd-key pointer-events-none inline-flex h-6 w-fit min-w-6 items-center justify-center gap-1 rounded-sm px-1.5 text-[10.5px] select-none [&_svg:not([class*='size-'])]:size-3",
        className
      )}
      {...props}
    />
  )
}

function KbdGroup({ className, ...props }: ComponentProps<'div'>): React.JSX.Element {
  return (
    <kbd
      data-slot="kbd-group"
      className={cn('inline-flex items-center gap-1', className)}
      {...props}
    />
  )
}

export { Kbd, KbdGroup }
