import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex cursor-default items-center justify-center gap-1.5 whitespace-nowrap rounded-md text-[13px] font-medium select-none disabled:pointer-events-none disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:size-3.5 [&_svg]:shrink-0',
  {
    variants: {
      /* default = blue block, secondary / outline = hairline outline (.btn in globals.css) */
      variant: {
        default: 'btn',
        secondary: 'btn btn-secondary',
        ghost: 'text-muted-foreground transition-colors hover:bg-accent hover:text-foreground',
        outline: 'btn btn-secondary',
        destructive: 'text-destructive transition-colors hover:bg-destructive/15'
      },
      size: {
        default: 'h-8 px-3',
        sm: 'h-7 px-2.5 text-[12px]',
        lg: 'h-10 px-5 text-[15px]',
        xl: 'h-13 px-6 text-[19px] [&_svg]:size-5',
        icon: 'size-8'
      }
    },
    defaultVariants: {
      variant: 'default',
      size: 'default'
    }
  }
)

function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }): React.JSX.Element {
  const Comp = asChild ? Slot : 'button'

  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
