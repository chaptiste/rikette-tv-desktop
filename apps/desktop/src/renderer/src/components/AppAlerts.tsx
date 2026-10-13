import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle
} from '@/components/ui/alert-dialog'
import { Toaster } from '@/components/ui/sonner'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import type { AppConfirm, AppToast } from '../../../tauri/api'

export function AppAlerts(): React.JSX.Element {
  const [ask, setAsk] = useState<AppConfirm | null>(null)
  const askRef = useRef<AppConfirm | null>(null)
  const replied = useRef(false)
  useEffect(() => {
    askRef.current = ask
  }, [ask])

  useEffect(() => {
    if (!hasDesktopApi()) return
    const offConfirm = ipc.onConfirm((next) => {
      replied.current = false
      setAsk(next)
    })
    const offToast = ipc.onToast(showToast)
    return () => {
      offConfirm()
      offToast()
    }
  }, [])

  const reply = (ok: boolean): void => {
    const current = askRef.current
    if (!current || replied.current) return
    replied.current = true
    ipc.replyConfirm(current.id, ok)
    setAsk(null)
  }

  return (
    <>
      <Toaster position="bottom-right" />
      <AlertDialog
        open={Boolean(ask)}
        onOpenChange={(open) => {
          if (!open) reply(false)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{ask?.title}</AlertDialogTitle>
            <AlertDialogDescription className="whitespace-pre-line">
              {ask?.description}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => reply(false)}>
              {ask?.cancelLabel ?? 'Cancel'}
            </AlertDialogCancel>
            <AlertDialogAction
              className={ask?.destructive ? 'btn-danger' : undefined}
              onClick={() => reply(true)}
            >
              {ask?.confirmLabel ?? 'Continue'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

function showToast(notice: AppToast): void {
  const opts = notice.description ? { description: notice.description } : undefined
  if (notice.kind === 'success') toast.success(notice.title, opts)
  else if (notice.kind === 'error') toast.error(notice.title, opts)
  else if (notice.kind === 'warning') toast.warning(notice.title, opts)
  else toast(notice.title, opts)
}
