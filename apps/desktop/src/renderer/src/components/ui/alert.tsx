import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const alertVariants = cva(
  'relative grid w-full grid-cols-[0_1fr] items-start gap-x-2 rounded-sm border-0 border-l-2 px-3 py-2.5 text-left text-[13px] has-[>svg]:grid-cols-[auto_1fr] has-[>svg]:gap-x-2 [&>svg]:size-3.5 [&>svg]:translate-y-0.5 [&>svg]:text-current',
  {
    variants: {
      variant: {
        default: 'border-primary bg-secondary text-card-foreground',
        destructive:
          'border-destructive bg-destructive/10 text-destructive [&>svg]:text-destructive'
      }
    },
    defaultVariants: {
      variant: 'default'
    }
  }
)

function Alert({
  className,
  variant,
  ...props
}: React.ComponentProps<'div'> & VariantProps<typeof alertVariants>): React.JSX.Element {
  return (
    <div
      data-slot="alert"
      role="alert"
      className={cn(alertVariants({ variant }), className)}
      {...props}
    />
  )
}

function AlertTitle({ className, ...props }: React.ComponentProps<'div'>): React.JSX.Element {
  return <div data-slot="alert-title" className={cn('font-semibold', className)} {...props} />
}

function AlertDescription({ className, ...props }: React.ComponentProps<'div'>): React.JSX.Element {
  return (
    <div
      data-slot="alert-description"
      className={cn('text-[12px] text-muted-foreground [&_p]:leading-5', className)}
      {...props}
    />
  )
}

export { Alert, AlertTitle, AlertDescription }
