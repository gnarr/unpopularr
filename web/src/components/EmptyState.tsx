import type { ReactNode } from 'react'

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string
  description?: string
  action?: ReactNode
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-slate-800 bg-slate-900/40 px-6 py-16 text-center">
      <h3 className="text-base font-semibold text-slate-200">{title}</h3>
      {description && <p className="max-w-md text-sm text-slate-400">{description}</p>}
      {action}
    </div>
  )
}
