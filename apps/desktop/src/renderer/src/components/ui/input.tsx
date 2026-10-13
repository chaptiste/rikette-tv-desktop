import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function Input({ className, type, ...props }: ComponentProps<'input'>): React.JSX.Element {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        'h-9 w-full min-w-0 rounded-md border border-border bg-input px-2.5 text-[13px] text-foreground outline-none transition-[border-color,box-shadow]',
        'placeholder:text-muted-foreground hover:border-border-strong focus-visible:border-primary',
        'disabled:opacity-40 file:border-0 file:bg-transparent file:text-[13px]',
        className
      )}
      {...props}
    />
  )
}

export { Input }
