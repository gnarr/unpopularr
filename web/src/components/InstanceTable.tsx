import type { InstanceReference } from '../api/types'
import { useInstanceMap } from '../api/queries'
import { arrName, deepLinkHref } from '../lib/deepLink'
import { formatBytes } from '../lib/format'
import { ArrLinkIcon } from './ArrLinkIcon'

export interface InstanceRow {
  instance: InstanceReference
  sizeOnDiskBytes: number
  fileCount: number
  // Rendered under `extraHeader` when provided (e.g. season numbers, album count).
  extra?: string
}

// The per-instance breakdown on detail pages. Single-instance setups skip it —
// the breakdown would just repeat the header stats. Each row's instance carries
// its own "open in the *arr web UI" deep link.
export function InstanceTable({ rows, extraHeader }: { rows: InstanceRow[]; extraHeader?: string }) {
  const instanceMap = useInstanceMap()

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
            {rows.map((row) => {
              const link = instanceMap.get(row.instance.id)
              const href = link ? deepLinkHref(link.externalUrl, row.instance.deepLinkPath) : null
              return (
              <tr key={row.instance.id} className="hover:bg-slate-800/30">
                <td className="border-b border-slate-800/60 px-3 py-1.5 text-slate-200">
                  <span className="inline-flex items-center gap-1.5">
                    {row.instance.name}
                    {href && link && (
                      <ArrLinkIcon href={href} label={`Open in ${arrName(link.kind)}`} />
                    )}
                  </span>
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
              )
            })}
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
