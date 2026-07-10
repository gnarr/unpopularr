import type { InstanceReference } from '../api/types'
import { formatBytes } from '../lib/format'

export interface InstanceRow {
  instance: InstanceReference
  sizeOnDiskBytes: number
  fileCount: number
  // Rendered under `extraHeader` when provided (e.g. season numbers, album count).
  extra?: string
}

// The per-instance breakdown on detail pages. Single-instance setups skip it —
// the breakdown would just repeat the header stats.
export function InstanceTable({ rows, extraHeader }: { rows: InstanceRow[]; extraHeader?: string }) {
  if (rows.length <= 1) return null

  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/40">
      <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-100">Instances</h2>
      </header>
      <div className="p-4">
        <table className="w-full border-separate border-spacing-0 text-sm">
          <thead>
            <tr>
              <HeaderCell align="left">Instance</HeaderCell>
              <HeaderCell align="right">Size</HeaderCell>
              <HeaderCell align="right">Files</HeaderCell>
              {extraHeader !== undefined && <HeaderCell align="left">{extraHeader}</HeaderCell>}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.instance.id} className="hover:bg-slate-800/30">
                <td className="border-b border-slate-800/60 px-3 py-1.5 text-slate-200">
                  {row.instance.name}
                </td>
                <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                  {formatBytes(row.sizeOnDiskBytes)}
                </td>
                <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                  {row.fileCount.toLocaleString()}
                </td>
                {extraHeader !== undefined && (
                  <td className="border-b border-slate-800/60 px-3 py-1.5 text-slate-400">
                    {row.extra || '—'}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}

function HeaderCell({ align, children }: { align: 'left' | 'right'; children: string }) {
  const alignment = align === 'right' ? 'text-right' : 'text-left'
  return (
    <th
      className={`border-b border-slate-800 px-3 py-2 ${alignment} text-xs font-semibold uppercase tracking-wide text-slate-400`}
    >
      {children}
    </th>
  )
}
