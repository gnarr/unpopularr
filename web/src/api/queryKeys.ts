export const queryKeys = {
  content: ['content'] as const,
  // `seriesAll` is the prefix that invalidates every cached series detail at once.
  seriesAll: ['series'] as const,
  series: (tvdbId: number) => ['series', tvdbId] as const,
  sync: ['sync'] as const,
  playbackSync: ['playbackSync'] as const,
}
