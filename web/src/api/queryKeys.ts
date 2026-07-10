export const queryKeys = {
  content: ['content'] as const,
  // The `*All` keys are prefixes that invalidate every cached detail at once.
  seriesAll: ['series'] as const,
  series: (tvdbId: number) => ['series', tvdbId] as const,
  movieAll: ['movie'] as const,
  movie: (tmdbId: number) => ['movie', tmdbId] as const,
  artistAll: ['artist'] as const,
  artist: (musicBrainzId: string) => ['artist', musicBrainzId] as const,
  sync: ['sync'] as const,
  playbackSync: ['playbackSync'] as const,
}
