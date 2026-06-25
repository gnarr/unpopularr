import { PlaybackSyncPanel } from './PlaybackSyncPanel'
import { SyncPanel } from './SyncPanel'

export function ActivityView() {
  return (
    <div className="mx-auto max-w-3xl space-y-6">
      <SyncPanel />
      <PlaybackSyncPanel />
    </div>
  )
}
