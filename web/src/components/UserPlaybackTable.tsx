import type { UserPlayback } from '../api/types'
import { absoluteTime, relativeTime } from '../lib/format'

// The per-user watch breakdown on detail pages. Hidden only when there's
// nothing to show — no attributed users and no unattributed plays. Rows synced
// before user tracking gain attribution on the next playback sync; plays the
// backend can't attribute to a user are summarized in the footnote, which
// stands alone when every play is unattributed.
export function UserPlaybackTable({
  users,
  unknownPlayCount,
}: {
  users: UserPlayback[]
  unknownPlayCount: number | null
}) {
  const hasUnknownPlays = unknownPlayCount !== null && unknownPlayCount > 0
  if (users.length === 0 && !hasUnknownPlays) return null

  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/40">
      <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-100">Watched by</h2>
      </header>
      <div className="p-4">
        {users.length > 0 && (
          <table className="w-full border-separate border-spacing-0 text-sm">
            <thead>
              <tr>
                <HeaderCell align="left">User</HeaderCell>
                <HeaderCell align="right">Plays</HeaderCell>
                <HeaderCell align="right">Last watched</HeaderCell>
              </tr>
            </thead>
            <tbody>
              {users.map((user) => (
                <tr key={user.userId} className="hover:bg-slate-800/30">
                  <td className="border-b border-slate-800/60 px-3 py-1.5 text-slate-200">
                    {user.userName ?? <span className="italic text-slate-500">Unknown</span>}
                  </td>
                  <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                    {user.playback.playCount.toLocaleString()}
                  </td>
                  <td className="border-b border-slate-800/60 px-3 py-1.5 text-right text-slate-400">
                    {user.playback.lastPlayedAt === null ? (
                      '—'
                    ) : (
                      <span title={absoluteTime(user.playback.lastPlayedAt)}>
                        {relativeTime(user.playback.lastPlayedAt)}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {hasUnknownPlays && (
          <p className={`text-xs text-slate-500${users.length > 0 ? ' mt-3' : ''}`}>
            {unknownPlayCount.toLocaleString()}{' '}
            {unknownPlayCount === 1 ? "play isn't" : "plays aren't"} attributed to a user (synced
            before user tracking or no longer in Tautulli's history).
          </p>
        )}
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
