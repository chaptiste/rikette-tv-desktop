import * as SliderPrimitive from '@radix-ui/react-slider'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function Slider({
  className,
  ...props
}: ComponentProps<typeof SliderPrimitive.Root>): React.JSX.Element {
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn(
        'relative flex h-7 w-full touch-none items-center select-none data-[disabled]:opacity-40',
        className
      )}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-1 w-full grow overflow-hidden bg-border-strong">
        <SliderPrimitive.Range className="absolute h-full bg-primary" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb className="block h-4 w-1.5 shrink-0 rounded-[1px] bg-foreground outline-none transition-transform hover:scale-y-125 focus-visible:ring-2 focus-visible:ring-ring" />
    </SliderPrimitive.Root>
  )
}

export { Slider }
