import type { ReactNode } from 'react'
import type { View } from '../App'
import { SyncBadge } from './SyncBadge'

export function Header({ view, onChange }: { view: View; onChange: (view: View) => void }) {
  return (
    <header className="sticky top-0 z-20 border-b border-slate-800 bg-slate-950/80 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-screen-2xl items-center gap-6 px-4 sm:px-6">
        <span className="text-lg font-semibold tracking-tight text-slate-100">
          unpopular<span className="text-indigo-400">r</span>
        </span>
        <nav className="flex gap-1">
          <Tab active={view === 'catalog'} onClick={() => onChange('catalog')}>
            Catalog
          </Tab>
          <Tab active={view === 'activity'} onClick={() => onChange('activity')}>
            Activity
          </Tab>
        </nav>
        <div className="ml-auto">
          <SyncBadge />
        </div>
      </div>
    </header>
  )
}

function Tab({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-3 py-1.5 text-sm font-medium transition ${
        active ? 'bg-slate-800 text-slate-100' : 'text-slate-400 hover:text-slate-200'
      }`}
    >
      {children}
    </button>
  )
}
