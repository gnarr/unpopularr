export function StatCard({
  label,
  value,
  accent,
  hint,
}: {
  label: string
  value: string
  accent?: boolean
  // Shown as a native tooltip on the whole card.
  hint?: string
}) {
  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/40 px-4 py-3" title={hint}>
      <div className="text-xs uppercase tracking-wide text-slate-500">{label}</div>
      <div className={`mt-1 text-xl font-semibold ${accent ? 'text-amber-300' : 'text-slate-100'}`}>
        {value}
      </div>
    </div>
  )
}
