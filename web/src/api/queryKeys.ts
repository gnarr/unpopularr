export const queryKeys = {
  content: ['content'] as const,
  series: (tvdbId: number) => ['series', tvdbId] as const,
  sync: ['sync'] as const,
  playbackSync: ['playbackSync'] as const,
}
