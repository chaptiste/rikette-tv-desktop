import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function Table({ className, ...props }: ComponentProps<'table'>): React.JSX.Element {
  return (
    <table
      data-slot="table"
      className={cn('w-full caption-bottom text-[13px]', className)}
      {...props}
    />
  )
}

function TableHeader({ className, ...props }: ComponentProps<'thead'>): React.JSX.Element {
  return (
    <thead
      data-slot="table-header"
      className={cn('sticky top-0 z-10 bg-background [&_tr]:border-b', className)}
      {...props}
    />
  )
}

function TableBody({ className, ...props }: ComponentProps<'tbody'>): React.JSX.Element {
  return (
    <tbody
      data-slot="table-body"
      className={cn('[&_tr:last-child]:border-0', className)}
      {...props}
    />
  )
}

function TableFooter({ className, ...props }: ComponentProps<'tfoot'>): React.JSX.Element {
  return (
    <tfoot
      data-slot="table-footer"
      className={cn('border-t bg-muted/50 font-medium [&>tr]:last:border-b-0', className)}
      {...props}
    />
  )
}

function TableRow({ className, ...props }: ComponentProps<'tr'>): React.JSX.Element {
  return (
    <tr
      data-slot="table-row"
      className={cn(
        'border-b border-border transition-colors hover:bg-accent/60 data-[state=selected]:bg-accent',
        className
      )}
      {...props}
    />
  )
}

function TableHead({ className, ...props }: ComponentProps<'th'>): React.JSX.Element {
  return (
    <th
      data-slot="table-head"
      className={cn('eyebrow h-8 px-3 text-left align-middle whitespace-nowrap', className)}
      {...props}
    />
  )
}

function TableCell({ className, ...props }: ComponentProps<'td'>): React.JSX.Element {
  return (
    <td data-slot="table-cell" className={cn('px-3 py-1.5 align-middle', className)} {...props} />
  )
}

function TableCaption({ className, ...props }: ComponentProps<'caption'>): React.JSX.Element {
  return (
    <caption
      data-slot="table-caption"
      className={cn('mt-4 text-[13px] text-muted-foreground', className)}
      {...props}
    />
  )
}

export { Table, TableHeader, TableBody, TableFooter, TableHead, TableRow, TableCell, TableCaption }
