import { useEffect, useRef, useState } from 'react'
import {
  FREE_CLOUD_CLIPS,
  PRO_BYTES,
  PRO_PRICE_MONTHLY_USD,
  PRO_PRICE_YEARLY_USD,
  type BillingInterval,
  type Me
} from '@echoframe/shared'
import { Check, Cloud, Crown, Link2, LogOut, Zap } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Progress } from '@/components/ui/progress'
import { PageHeader } from '@/components/PageHeader'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { cn } from '@/lib/utils'

const PRO_GB = Math.round(PRO_BYTES / 1024 ** 3)
const YEARLY_SAVING = Math.round((1 - PRO_PRICE_YEARLY_USD / (PRO_PRICE_MONTHLY_USD * 12)) * 100)

export function Account(): React.JSX.Element {
  const [me, setMe] = useState<Me | null>(null)
  const [apiBase, setApiBase] = useState('http://127.0.0.1:8787')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.getSettings().then((settings) => setApiBase(settings.apiBase))
    void ipc.getMe().then(setMe)
    void ipc
      .refreshMe()
      .then(setMe)
      .catch(() => setMe(null))
    const onVis = (): void => {
      if (document.visibilityState !== 'visible') return
      void ipc
        .refreshMe()
        .then(setMe)
        .catch(() => undefined)
    }
    document.addEventListener('visibilitychange', onVis)
    window.addEventListener('focus', onVis)
    const unsub = ipc.onAuthChanged(setMe)
    return () => {
      document.removeEventListener('visibilitychange', onVis)
      window.removeEventListener('focus', onVis)
      unsub()
      if (pollRef.current) clearInterval(pollRef.current)
    }
  }, [])

  function watchPlan(): void {
    if (pollRef.current) clearInterval(pollRef.current)
    let ticks = 0
    pollRef.current = setInterval(() => {
      ticks += 1
      void ipc
        .refreshMe()
        .then(setMe)
        .catch(() => undefined)
      if (ticks >= 48 && pollRef.current) {
        clearInterval(pollRef.current)
        pollRef.current = null
      }
    }, 2500)
  }

  async function runBilling(action: () => Promise<void>): Promise<void> {
    setBusy(true)
    setError(null)
    try {
      await action()
      watchPlan()
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  function checkout(interval: BillingInterval): Promise<void> {
    return runBilling(() => ipc.startCheckout(interval))
  }

  const usedPct =
    me && me.storageCap > 0 ? Math.min(100, Math.round((me.usedBytes / me.storageCap) * 100)) : 0
  const name = me ? (me.email ?? me.id) : ''
  const pro = me?.plan === 'pro'

  return (
    <main className="flex h-full flex-col">
      <PageHeader kicker={me ? 'Your account' : 'Account'} title="Profile" />
      <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-10">
        {me ? (
          <div className="flex max-w-3xl flex-col gap-6">
            <section className="card rise-in flex items-center gap-6 p-6">
              <div className="relative shrink-0">
                <div className="display-title relative grid size-20 place-items-center bg-primary text-[44px] text-primary-foreground uppercase">
                  {name.charAt(0)}
                </div>
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'tc inline-flex h-5 items-center gap-1 rounded-sm px-1.5 text-[10px] font-semibold uppercase',
                      pro ? 'bg-warn text-background' : 'bg-secondary text-muted-foreground'
                    )}
                  >
                    {pro ? <Crown className="size-3" /> : null}
                    {pro ? 'Pro' : 'Basic'}
                  </span>
                  {!pro ? (
                    <span className="eyebrow">
                      {me.clipCount}/{me.freeClipLimit} cloud clips
                    </span>
                  ) : null}
                </div>
                <p className="display-title mt-2 truncate text-[28px]">{name}</p>
                <div className="mt-4">
                  <div className="mb-1.5 flex justify-between">
                    <span className="eyebrow">Cloud storage</span>
                    <span className="tc text-[10.5px] text-muted-foreground">
                      {formatGb(me.usedBytes)} / {formatGb(me.storageCap)} GB
                    </span>
                  </div>
                  <Progress value={usedPct} className="h-2" />
                </div>
              </div>
              <div className="flex shrink-0 flex-col gap-3 self-start">
                {pro ? (
                  <Button
                    type="button"
                    disabled={busy}
                    onClick={() => void runBilling(() => ipc.openBillingPortal())}
                  >
                    Manage billing
                  </Button>
                ) : null}
                <Button type="button" variant="outline" onClick={() => void ipc.signOut()}>
                  <LogOut />
                  Sign out
                </Button>
              </div>
            </section>

            {!pro ? (
              <section className="flex flex-col gap-3">
                <p className="eyebrow">Upgrade</p>
                <div className="grid grid-cols-2 gap-4">
                  <Tier
                    i={1}
                    name="Pro monthly"
                    price={`€${PRO_PRICE_MONTHLY_USD}`}
                    per="/ month"
                    busy={busy}
                    onPick={() => void checkout('month')}
                  />
                  <Tier
                    i={2}
                    name="Pro yearly"
                    price={`€${PRO_PRICE_YEARLY_USD}`}
                    per="/ year"
                    tag={YEARLY_SAVING > 0 ? `Save ${YEARLY_SAVING}%` : undefined}
                    featured
                    busy={busy}
                    onPick={() => void checkout('year')}
                  />
                </div>
              </section>
            ) : null}

            {error ? (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
          </div>
        ) : (
          <section className="card rise-in relative flex max-w-xl flex-col items-start gap-4 overflow-hidden border-t-2 border-t-primary p-8">
            <p className="eyebrow">Optional</p>
            <h2 className="display-title text-[38px]">Sign in to share clips</h2>
            <ul className="flex flex-col gap-2 text-[13.5px] text-muted-foreground">
              <Perk icon={Link2}>A share link lands on your clipboard when you save</Perk>
              <Perk icon={Cloud}>{FREE_CLOUD_CLIPS} free cloud clips, no card needed</Perk>
              <Perk icon={Zap}>Recording keeps working offline, no account required</Perk>
            </ul>
            <div className="mt-2 flex gap-3">
              <Button
                type="button"
                size="lg"
                disabled={busy}
                onClick={() => {
                  setError(null)
                  void ipc.signIn()
                }}
              >
                Sign in with browser
              </Button>
              {apiBase.includes('localhost') || apiBase.includes('127.0.0.1') ? (
                <Button
                  type="button"
                  size="lg"
                  variant="outline"
                  disabled={busy}
                  onClick={() => {
                    setBusy(true)
                    setError(null)
                    void ipc
                      .devLogin('dev@echoframe.local')
                      .then(setMe)
                      .catch((err: unknown) => {
                        setError(err instanceof Error ? err.message : String(err))
                      })
                      .finally(() => setBusy(false))
                  }}
                >
                  Local dev login
                </Button>
              ) : null}
            </div>
            {error ? (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
          </section>
        )}
      </div>
    </main>
  )
}

function Tier(props: {
  i: number
  name: string
  price: string
  per: string
  tag?: string
  featured?: boolean
  busy: boolean
  onPick: () => void
}): React.JSX.Element {
  return (
    <div
      className={cn(
        'card rise-in group relative flex flex-col gap-4 p-5 transition-colors duration-150',
        props.featured ? 'border-t-2 border-t-primary' : 'hover:border-border-strong'
      )}
      style={{ '--i': props.i } as React.CSSProperties}
    >
      {props.tag ? (
        <span className="tc absolute top-4 right-4 bg-ok px-1.5 py-0.5 text-[10px] font-semibold text-background uppercase">
          {props.tag}
        </span>
      ) : null}
      <div>
        <p className="eyebrow">{props.name}</p>
        <p className="mt-2 flex items-baseline gap-1.5">
          <span className="display-title text-[48px]">{props.price}</span>
          <span className="tc text-[11px] text-muted-foreground">{props.per}</span>
        </p>
      </div>
      <ul className="flex flex-col gap-1.5 text-[13px] text-muted-foreground">
        <Perk icon={Check}>{PRO_GB} GB of cloud storage</Perk>
        <Perk icon={Check}>No cap on cloud clips</Perk>
      </ul>
      <Button
        type="button"
        size="lg"
        variant={props.featured ? 'default' : 'outline'}
        disabled={props.busy}
        onClick={props.onPick}
      >
        <Crown />
        Go Pro
      </Button>
    </div>
  )
}

function Perk(props: {
  icon: React.ComponentType<{ className?: string; strokeWidth?: number }>
  children: React.ReactNode
}): React.JSX.Element {
  const Icon = props.icon
  return (
    <li className="flex items-center gap-2">
      <Icon className="size-4 shrink-0 text-ok" strokeWidth={2.25} />
      {props.children}
    </li>
  )
}

function formatGb(bytes: number): string {
  return (bytes / 1024 ** 3).toFixed(bytes >= 1024 ** 3 ? 1 : 2)
}
