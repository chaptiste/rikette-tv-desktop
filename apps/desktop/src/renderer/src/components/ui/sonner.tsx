import type { CSSProperties, JSX } from 'react'
import { Toaster as Sonner, type ToasterProps } from 'sonner'
import { CircleCheck, Info, Loader2, OctagonX, TriangleAlert } from 'lucide-react'

function Toaster(props: ToasterProps): JSX.Element {
  return (
    <Sonner
      theme="dark"
      className="toaster no-drag group !z-40"
      icons={{
        success: <CircleCheck className="size-3.5" />,
        info: <Info className="size-3.5" />,
        warning: <TriangleAlert className="size-3.5" />,
        error: <OctagonX className="size-3.5" />,
        loading: <Loader2 className="size-3.5 animate-spin" />
      }}
      style={
        {
          '--normal-bg': 'var(--popover)',
          '--normal-text': 'var(--popover-foreground)',
          '--normal-border': 'var(--border)',
          '--border-radius': 'var(--radius)'
        } as CSSProperties
      }
      toastOptions={{
        classNames: {
          toast:
            'rounded-md border border-border-strong bg-popover text-[13px] text-popover-foreground shadow-[0_16px_40px_-12px_var(--shadow)]',
          title: 'font-semibold',
          success: '[&_[data-icon]]:text-ok',
          error: '[&_[data-icon]]:text-destructive',
          warning: '[&_[data-icon]]:text-warn'
        }
      }}
      {...props}
    />
  )
}

export { Toaster }
