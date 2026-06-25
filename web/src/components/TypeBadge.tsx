import type { ContentType } from '../api/types'

const META: Record<ContentType, { label: string; icon: string; className: string }> = {
  movie: { label: 'Movie', icon: '🎬', className: 'bg-sky-500/15 text-sky-300' },
  series: { label: 'Series', icon: '📺', className: 'bg-violet-500/15 text-violet-300' },
  artist: { label: 'Artist', icon: '🎵', className: 'bg-emerald-500/15 text-emerald-300' },
}

export function TypeBadge({ type }: { type: ContentType }) {
  const meta = META[type]
  return (
    <span
      className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-xs font-medium ${meta.className}`}
    >
      <span aria-hidden>{meta.icon}</span>
      {meta.label}
    </span>
  )
}
