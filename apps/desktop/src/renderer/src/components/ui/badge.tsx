import { cva, type VariantProps } from 'class-variance-authority'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

const badgeVariants = cva(
  'inline-flex h-5 w-fit shrink-0 items-center justify-center gap-1 rounded-sm border px-1.5 text-[10px] font-medium uppercase tabular-nums whitespace-nowrap leading-none',
  {
    variants: {
      variant: {
        default: 'border-transparent bg-primary text-primary-foreground',
        secondary: 'border-transparent bg-[var(--scrim)] text-ok',
        outline: 'border-transparent bg-[var(--scrim)] text-foreground',
        destructive: 'border-transparent bg-rec text-primary-foreground'
      }
    },
    defaultVariants: {
      variant: 'default'
    }
  }
)

function Badge({
  className,
  variant,
  ...props
}: ComponentProps<'span'> & VariantProps<typeof badgeVariants>): React.JSX.Element {
  return <span data-slot="badge" className={cn(badgeVariants({ variant }), className)} {...props} />
}

export { Badge, badgeVariants }
