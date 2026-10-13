import * as ToggleGroupPrimitive from '@radix-ui/react-toggle-group'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function ToggleGroup({
  className,
  ...props
}: ComponentProps<typeof ToggleGroupPrimitive.Root>): React.JSX.Element {
  return (
    <ToggleGroupPrimitive.Root
      data-slot="toggle-group"
      className={cn('flex rounded-md border border-border-strong', className)}
      {...props}
    />
  )
}

function ToggleGroupItem({
  className,
  ...props
}: ComponentProps<typeof ToggleGroupPrimitive.Item>): React.JSX.Element {
  return (
    <ToggleGroupPrimitive.Item
      data-slot="toggle-group-item"
      className={cn(
        'h-7 px-3 text-[12.5px] font-medium text-muted-foreground outline-none transition-colors duration-100 not-first:border-l not-first:border-border-strong',
        'hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring',
        'data-[state=on]:bg-primary data-[state=on]:text-primary-foreground',
        className
      )}
      {...props}
    />
  )
}

export { ToggleGroup, ToggleGroupItem }
