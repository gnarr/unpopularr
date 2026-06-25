import { useStartSync } from '../api/mutations'
import { useSyncStatus } from '../api/queries'
import type { SyncRun } from '../api/types'
import { Button } from '../components/Button'
import { StatusChip } from '../components/StatusChip'
import { absoluteTime, elapsedSeconds, formatDuration, relativeTime } from '../lib/format'
import { InstanceResultRow } from './InstanceResultRow'

export function SyncPanel() {
  const sync = useSyncStatus()
  const start = useStartSync()
  const run = sync.data ?? null
  const running = run?.status === 'running'

  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/40">
      <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-100">Library sync</h2>
        <Button onClick={() => start.mutate()} disabled={running || start.isPending}>
          {running ? 'Syncing…' : start.isPending ? 'Starting…' : 'Run sync now'}
        </Button>
      </header>
      <div className="p-4">
        {sync.isPending ? (
          <p className="text-sm text-slate-500">Loading…</p>
        ) : !run ? (
          <p className="text-sm text-slate-400">No sync has run yet.</p>
        ) : (
          <div className="space-y-4">
            <RunSummary run={run} />
            {run.instances.length > 0 && (
              <div className="divide-y divide-slate-800/60">
                {run.instances.map((instance) => (
                  <InstanceResultRow key={instance.id} result={instance} />
                ))}
              </div>
            )}
          </div>
        )}
        {start.isError && (
          <p className="mt-3 text-sm text-red-300">
            {start.error instanceof Error ? start.error.message : 'Failed to start sync'}
          </p>
        )}
      </div>
    </section>
  )
}

function RunSummary({ run }: { run: SyncRun }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm">
      <StatusChip status={run.status} />
      <Field label="Trigger" value={run.trigger} />
      <span className="text-slate-400">
        Started{' '}
        <span className="text-slate-200" title={absoluteTime(run.startedAt)}>
          {relativeTime(run.startedAt)}
        </span>
      </span>
      <Field
        label="Duration"
        value={run.completedAt ? formatDuration(elapsedSeconds(run.startedAt, run.completedAt)) : 'in progress'}
      />
      <Field label="Imported" value={run.importedItems.toLocaleString()} />
    </div>
  )
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <span className="text-slate-400">
      {label}: <span className="tabular-nums text-slate-200">{value}</span>
    </span>
  )
}
