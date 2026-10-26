import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { NavLink, useLocation, useNavigate } from 'react-router'
import { Minus, Square, X } from 'lucide-react'
import { AppLogo } from '@/components/AppLogo'
import { RecChip } from '@/components/RecChip'
import { cn } from '@/lib/utils'
import { hasDesktopApi, ipc } from '@/lib/ipc'

const links = [
  { to: '/', label: 'Replay' },
  { to: '/library', label: 'Clips' },
  { to: '/editor', label: 'Editor' },
  { to: '/settings', label: 'Settings' },
  { to: '/account', label: 'Profile' }
] as const

function isActive(to: string, pathname: string): boolean {
  if (to === '/editor') return pathname === '/editor' || pathname.startsWith('/editor/')
  return pathname === to
}

/** Tabs across the title bar, each with its Ctrl+number. A blue rule slides
 * under the active tab. */
export function TopNav(): React.JSX.Element {
  const location = useLocation()
  const navigate = useNavigate()
  const mac = hasDesktopApi() && ipc.platform === 'darwin'
  const navRef = useRef<HTMLElement>(null)
  const [bar, setBar] = useState<{ left: number; width: number } | null>(null)

  useLayoutEffect(() => {
    const nav = navRef.current
    if (!nav) return
    const measure = (): void => {
      const active = nav.querySelector<HTMLElement>('[data-active="true"]')
      setBar(active ? { left: active.offsetLeft, width: active.offsetWidth } : null)
    }
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(nav)
    return () => observer.disconnect()
  }, [location.pathname])

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return
      const index = Number(event.key) - 1
      const link = links[index]
      if (!link) return
      event.preventDefault()
      navigate(link.to)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [navigate])

  return (
    <header
      className={cn(
        'drag relative z-30 flex h-11 shrink-0 items-stretch border-b border-border bg-sidebar',
        mac && 'pl-[76px]'
      )}
    >
      <div className="flex items-center gap-2.5 border-r border-border px-4">
        <AppLogo className="size-[22px]" />
        <span className="display-title text-[18px]">Rikette</span>
      </div>

      <nav ref={navRef} className="relative flex items-stretch">
        {links.map((link, index) => {
          const active = isActive(link.to, location.pathname)
          return (
            <NavLink
              key={link.to}
              to={link.to}
              data-active={active}
              title={`${link.label} (Ctrl+${index + 1})`}
              className={cn(
                'no-drag group flex items-center gap-2 px-4 font-display text-[14px] font-bold transition-colors duration-150',
                active ? 'text-foreground' : 'text-sidebar-muted hover:text-foreground'
              )}
            >
              {link.label}
              <span
                className={cn(
                  'tc text-[9.5px] font-medium transition-colors',
                  active ? 'text-primary' : 'text-border-strong group-hover:text-muted-foreground'
                )}
              >
                {index + 1}
              </span>
            </NavLink>
          )
        })}
        <span
          aria-hidden
          className="pointer-events-none absolute -bottom-px h-0.5 bg-primary transition-all duration-200 ease-[cubic-bezier(0.65,0,0.35,1)]"
          style={bar ? { left: bar.left + 16, width: bar.width - 32 } : { opacity: 0 }}
        />
      </nav>

      <div className={cn('ml-auto flex items-center pl-4', mac ? 'pr-3' : 'pr-2')}>
        <RecChip />
      </div>
      {!mac && hasDesktopApi() && <WindowButtons />}
    </header>
  )
}

/** Minimise, maximise and close, drawn by the page since the window has no frame. */
function WindowButtons(): React.JSX.Element {
  const button =
    'no-drag grid w-[46px] place-items-center text-sidebar-muted transition-colors duration-150 hover:text-foreground'
  return (
    <div className="flex items-stretch border-l border-border">
      <button
        type="button"
        aria-label="Minimise"
        className={cn(button, 'hover:bg-accent')}
        onClick={ipc.minimizeWindow}
      >
        <Minus className="size-4" strokeWidth={1.75} />
      </button>
      <button
        type="button"
        aria-label="Maximise"
        className={cn(button, 'hover:bg-accent')}
        onClick={ipc.toggleMaximizeWindow}
      >
        <Square className="size-3.5" strokeWidth={1.75} />
      </button>
      <button
        type="button"
        aria-label="Close"
        className={cn(button, 'hover:bg-rec')}
        onClick={ipc.closeWindow}
      >
        <X className="size-4" strokeWidth={1.75} />
      </button>
    </div>
  )
}
