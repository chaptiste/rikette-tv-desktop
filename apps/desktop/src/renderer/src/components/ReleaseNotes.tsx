import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '@/components/ui/dialog'
import { hasDesktopApi, ipc } from '@/lib/ipc'
import { notesBetween, type ReleaseNote } from '@/lib/releaseNotes'

/** Opens once after an update with the notes for the version(s) just installed. */
export function ReleaseNotes(): React.JSX.Element | null {
  const [notes, setNotes] = useState<ReleaseNote[]>([])
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (!hasDesktopApi()) return
    void ipc.takeVersionChange().then((change) => {
      if (!change) return
      const next = notesBetween(change.from, change.to)
      if (next.length === 0) return
      setNotes(next)
      setOpen(true)
    })
  }, [])

  const latest = notes[0]
  if (!latest) return null

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent>
        <DialogHeader>
          <p className="eyebrow">What&apos;s new in {latest.version}</p>
          <DialogTitle className="text-[34px]">{latest.title}</DialogTitle>
          <DialogDescription className="sr-only">
            Release notes for the version you just installed.
          </DialogDescription>
        </DialogHeader>
        <div className="-mx-4 flex max-h-[min(24rem,60vh)] flex-col gap-5 overflow-y-auto px-4">
          {notes.map((note, index) => (
            <section key={note.version} className="flex flex-col gap-2">
              {index > 0 ? (
                <h3 className="border-t border-border pt-4 font-display text-[16px] font-bold">
                  {note.title}
                  <span className="tc ml-2 text-[10px] font-normal text-muted-foreground">
                    {note.version}
                  </span>
                </h3>
              ) : null}
              <ul className="flex flex-col gap-2">
                {note.items.map((item, i) => (
                  <li
                    key={item}
                    className="rise-in flex gap-3 text-[13.5px] leading-5"
                    style={{ '--i': i + 2 } as React.CSSProperties}
                  >
                    <span className="mt-[7px] h-1.5 w-3 shrink-0 bg-primary" />
                    <span className="text-foreground/90">{item}</span>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button size="lg">Got it</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
