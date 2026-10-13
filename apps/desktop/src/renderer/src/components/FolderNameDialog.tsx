import { useState } from 'react'
import { Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { ipcErrorMessage } from '@/lib/ipc'

interface FolderNameProps {
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  confirmLabel: string
  initialName?: string
  onSubmit: (name: string) => Promise<void>
}

/** Names a new folder or renames one. `onSubmit` throws to keep the dialog open with its message. */
export function FolderNameDialog(props: FolderNameProps & { open: boolean }): React.JSX.Element {
  const { open, ...form } = props
  return (
    <Dialog open={open} onOpenChange={props.onOpenChange}>
      {/* The content unmounts on close, so the form starts fresh every time it opens. */}
      <DialogContent>
        <FolderNameForm {...form} />
      </DialogContent>
    </Dialog>
  )
}

function FolderNameForm(props: FolderNameProps): React.JSX.Element {
  const [name, setName] = useState(props.initialName ?? '')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (): Promise<void> => {
    if (busy) return
    if (!name.trim()) {
      setError('Give the folder a name')
      return
    }
    setBusy(true)
    try {
      await props.onSubmit(name)
      props.onOpenChange(false)
    } catch (err) {
      setError(ipcErrorMessage(err))
      setBusy(false)
    }
  }

  return (
    <form
      className="grid gap-4"
      onSubmit={(event) => {
        event.preventDefault()
        void submit()
      }}
    >
      <DialogHeader>
        <DialogTitle>{props.title}</DialogTitle>
        <DialogDescription>{props.description}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-1.5">
        <Input
          autoFocus
          value={name}
          maxLength={40}
          placeholder="Ranked, Squad nights, Funny…"
          aria-label="Folder name"
          aria-invalid={error ? true : undefined}
          spellCheck={false}
          onFocus={(event) => event.currentTarget.select()}
          onChange={(event) => {
            setName(event.target.value)
            setError(null)
          }}
        />
        {error ? <p className="text-[12px] text-destructive">{error}</p> : null}
      </div>
      <DialogFooter>
        <Button type="button" variant="secondary" onClick={() => props.onOpenChange(false)}>
          Cancel
        </Button>
        <Button type="submit" disabled={busy}>
          {busy ? <Loader2 className="animate-spin" /> : null}
          {props.confirmLabel}
        </Button>
      </DialogFooter>
    </form>
  )
}
