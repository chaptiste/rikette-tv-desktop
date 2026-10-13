import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function Label({ className, ...props }: ComponentProps<'label'>): React.JSX.Element {
  return <label className={cn('text-[13px] text-muted-foreground', className)} {...props} />
}

export { Label }
