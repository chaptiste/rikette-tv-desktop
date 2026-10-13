import { useNavigate } from 'react-router'
import { recMode, useCaptureStatus, type RecMode } from '@/lib/capture'
import { cn } from '@/lib/utils'

const light: Record<RecMode, { label: string; className: string; live: boolean }> = {
  recording: { label: 'REC', className: 'text-rec', live: true },
  saving: { label: 'SAVING', className: 'text-highlight', live: true },
  starting: { label: 'STARTING', className: 'text-warn', live: true },
  paused: { label: 'PAUSED', className: 'text-warn', live: false },
  offline: { label: 'OFFLINE', className: 'text-muted-foreground', live: false }
}

/** Always-visible buffer status in the title bar, like a camera's REC light. */
export function RecChip(): React.JSX.Element {
  const navigate = useNavigate()
  const [capture] = useCaptureStatus()
  const mode = recMode(capture)
  const view = light[mode]

  return (
    <button
      type="button"
      onClick={() => navigate('/')}
      className={cn(
        'no-drag tc flex h-7 items-center gap-2 px-2 text-[11px] font-semibold transition-opacity hover:opacity-80',
        view.className
      )}
    >
      {view.live ? (
        <span className="live-dot" />
      ) : (
        <span className="size-2 rounded-full bg-current opacity-60" />
      )}
      <span className="text-foreground">{view.label}</span>
      {mode === 'recording' && capture ? (
        <span className="font-medium text-muted-foreground">{capture.bufferSec}s</span>
      ) : null}
    </button>
  )
}
