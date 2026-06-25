import type { InstanceSyncResult } from '../api/types'
import { StatusChip } from '../components/StatusChip'
import { absoluteTime, relativeTime } from '../lib/format'

export function InstanceResultRow({ result }: { result: InstanceSyncResult }) {
  return (
    <div className="py-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
        <span className="font-medium text-slate-200">{result.name}</span>
        <span className="text-xs uppercase tracking-wide text-slate-500">{result.kind}</span>
        <StatusChip status={result.status} />
        <span className="text-slate-400">
          Imported <span className="tabular-nums text-slate-200">{result.importedItems.toLocaleString()}</span>
        </span>
        <span className="ml-auto text-slate-500" title={absoluteTime(result.startedAt)}>
          {relativeTime(result.startedAt)}
        </span>
      </div>
      {result.error && <p className="mt-1 text-xs text-red-300">{result.error}</p>}
    </div>
  )
}
