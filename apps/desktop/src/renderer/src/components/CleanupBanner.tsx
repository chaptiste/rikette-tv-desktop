import { useEffect, useState } from 'react'
import { useLocation, useNavigate } from 'react-router'
import type { Me } from '@echoframe/shared'
import { Button } from '@/components/ui/button'
import { hasDesktopApi, ipc } from '@/lib/ipc'

/** Under the top bar while Pro has ended and cloud clips over the free limit
 * are counting down to deletion. */
export function CleanupBanner(): React.JSX.Element | null {
  const [me, setMe] = useState<Me | null>(null)
  const [now, setNow] = useState(0)
  const navigate = useNavigate()
  const location = useLocation()

  useEffect(() => {
    if (!hasDesktopApi()) return
    const show = (next: Me | null): void => {
      setMe(next)
      setNow(Date.now())
    }
    void ipc.getMe().then(show)
    void ipc
      .refreshMe()
      .then(show)
      .catch(() => undefined)
    return ipc.onAuthChanged(show)
  }, [])

  if (!me?.cleanupAt || !me.cleanupClips) return null
  const clips = me.cleanupClips === 1 ? '1 cloud clip' : `${me.cleanupClips} cloud clips`
  const date = new Date(me.cleanupAt).toLocaleDateString(undefined, {
    day: 'numeric',
    month: 'long'
  })
  const days = Math.ceil((me.cleanupAt - now) / 86_400_000)

  return (
    <div
      role="alert"
      className="relative z-10 flex shrink-0 items-center gap-3 border-b border-l-2 border-border border-l-rec bg-card px-4 py-2 text-[13px]"
    >
      <span className="eyebrow shrink-0 text-rec">Pro ended</span>
      <p className="min-w-0 flex-1 text-muted-foreground">
        <span className="text-foreground">{clips}</span> will be deleted from the cloud on{' '}
        <span className="text-foreground">{date}</span>{' '}
        <span className="tc">({days <= 1 ? 'within a day' : `in ${days} days`})</span>, with their
        share links. Your {me.freeClipLimit} newest stay. Clips on this PC are not affected.
      </p>
      {location.pathname !== '/account' ? (
        <Button type="button" size="sm" className="shrink-0" onClick={() => navigate('/account')}>
          Renew Pro
        </Button>
      ) : null}
    </div>
  )
}
