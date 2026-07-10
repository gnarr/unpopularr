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

export function DetailSkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        {Array.from({ length: 4 }).map((_, index) => (
          <StatCardSkeleton key={index} />
        ))}
      </div>
      <TableSkeleton />
    </div>
  )
}
