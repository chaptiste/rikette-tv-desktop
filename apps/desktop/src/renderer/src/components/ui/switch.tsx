import * as SwitchPrimitive from '@radix-ui/react-switch'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function Switch({
  className,
  ...props
}: ComponentProps<typeof SwitchPrimitive.Root>): React.JSX.Element {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        'peer inline-flex h-5 w-9 shrink-0 items-center rounded-sm border border-transparent outline-none transition-colors duration-200',
        'focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40',
        'data-[state=checked]:bg-primary data-[state=unchecked]:bg-input data-[state=unchecked]:border-border-strong',
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block size-3.5 rounded-[2px] bg-foreground transition-transform duration-150 ease-out data-[state=checked]:translate-x-[18px] data-[state=checked]:bg-white data-[state=unchecked]:translate-x-[2px] data-[state=unchecked]:bg-muted-foreground"
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch }
