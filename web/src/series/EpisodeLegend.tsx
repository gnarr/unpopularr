const SWATCH = 'inline-block h-3 w-3 rounded'

function Entry({ swatch, label }: { swatch: React.ReactNode; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      {swatch}
      {label}
    </span>
  )
}

export function EpisodeLegend({ playbackAvailable }: { playbackAvailable: boolean }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-400">
      {playbackAvailable ? (
        <>
          <Entry
            swatch={
              <span className="flex gap-0.5">
                <span className={`${SWATCH} bg-indigo-500/70`} />
                <span className={`${SWATCH} bg-indigo-500/40`} />
                <span className={`${SWATCH} bg-indigo-500/20`} />
              </span>
            }
            label="Watched (bright = recent)"
          />
          <Entry
            swatch={
              <span className={`${SWATCH} bg-red-500/15 ring-1 ring-red-500/30 ring-inset`} />
            }
            label="Never watched"
          />
        </>
      ) : (
        <Entry swatch={<span className={`${SWATCH} bg-slate-700/60`} />} label="On disk" />
      )}
      <Entry
        swatch={<span className={`${SWATCH} border border-slate-700/60 bg-slate-800/40`} />}
        label="No file"
      />
      <Entry
        swatch={<span className={`${SWATCH} border border-dashed border-slate-700`} />}
        label="Unaired"
      />
    </div>
  )
}
