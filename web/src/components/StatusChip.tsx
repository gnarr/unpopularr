import type { SyncStatus } from '../api/types'

const STYLES: Record<SyncStatus, string> = {
  running: 'bg-blue-500/15 text-blue-300 ring-blue-500/30',
  succeeded: 'bg-green-500/15 text-green-300 ring-green-500/30',
  partial: 'bg-amber-500/15 text-amber-300 ring-amber-500/30',
  failed: 'bg-red-500/15 text-red-300 ring-red-500/30',
}

const LABELS: Record<SyncStatus, string> = {
  running: 'Running',
  succeeded: 'Succeeded',
  partial: 'Partial',
  failed: 'Failed',
}

export function StatusChip({ status }: { status: SyncStatus }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium ring-1 ring-inset ${STYLES[status]}`}
    >
      {status === 'running' && (
        <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />
      )}
      {LABELS[status]}
    </span>
  )
}
