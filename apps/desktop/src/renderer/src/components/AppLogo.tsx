import logo from '@/assets/logo.png'
import { cn } from '@/lib/utils'

export function AppLogo({ className }: { className?: string }): React.JSX.Element {
  return <img src={logo} alt="" draggable={false} className={cn('select-none', className)} />
}
