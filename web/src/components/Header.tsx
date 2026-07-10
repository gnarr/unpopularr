import type { ReactNode } from 'react'
import { NavLink, useLocation } from 'react-router'
import { SyncBadge } from './SyncBadge'

export function Header() {
  const { pathname } = useLocation()
  // Detail pages drill in from the catalog, so keep the Catalog tab lit.
  const onCatalog =
    pathname === '/' ||
    ['/series', '/movies', '/artists'].some((prefix) => pathname.startsWith(prefix))

  return (
    <header className="sticky top-0 z-20 border-b border-slate-800 bg-slate-950/80 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-screen-2xl items-center gap-6 px-4 sm:px-6">
        <span className="text-lg font-semibold tracking-tight text-slate-100">
          unpopular<span className="text-indigo-400">r</span>
        </span>
        <nav className="flex gap-1">
          <Tab to="/" active={onCatalog}>
            Catalog
          </Tab>
          <Tab to="/activity">Activity</Tab>
        </nav>
        <div className="ml-auto">
          <SyncBadge />
        </div>
      </div>
    </header>
  )
}

function Tab({ to, active, children }: { to: string; active?: boolean; children: ReactNode }) {
  const className = (isActive: boolean) =>
    `rounded-md px-3 py-1.5 text-sm font-medium transition ${
      isActive ? 'bg-slate-800 text-slate-100' : 'text-slate-400 hover:text-slate-200'
    }`
  return (
    <NavLink to={to} end className={({ isActive }) => className(active ?? isActive)}>
      {children}
    </NavLink>
  )
}
