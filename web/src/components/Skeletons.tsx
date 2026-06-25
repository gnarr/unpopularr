export function TableSkeleton() {
  return (
    <div className="space-y-2">
      {Array.from({ length: 10 }).map((_, index) => (
        <div key={index} className="h-9 animate-pulse rounded bg-slate-800/60" />
      ))}
    </div>
  )
}

export function StatCardSkeleton() {
  return <div className="h-[68px] animate-pulse rounded-lg bg-slate-800/60" />
}
