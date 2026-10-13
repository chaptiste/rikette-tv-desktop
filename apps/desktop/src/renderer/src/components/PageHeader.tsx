import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

/** Page title row under the top nav: mono kicker, big condensed title, actions on the right.
 * Without a title it is a plain toolbar row (the editor puts its own controls in it). */
export function PageHeader(props: {
  title?: ReactNode
  kicker?: ReactNode
  children?: ReactNode
  className?: string
}): React.JSX.Element {
  return (
    <header
      className={cn(
        'relative z-10 flex shrink-0 items-end gap-4 px-8 pt-6 pb-4',
        props.title ? null : 'items-center pt-3 pb-3',
        props.className
      )}
    >
      {props.title ? (
        <div className="slide-in-left min-w-0">
          {props.kicker ? <p className="eyebrow mb-1.5">{props.kicker}</p> : null}
          <h1 className="display-title text-[40px] text-foreground">{props.title}</h1>
        </div>
      ) : null}
      {props.children}
    </header>
  )
}
