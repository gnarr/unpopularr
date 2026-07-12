import { describe, expect, it } from 'vitest'
import type { InstanceLink } from '../api/types'
import { arrName, deepLinkHref, deepLinkPath } from './deepLink'

describe('deepLinkPath', () => {
  it('routes movies by TMDB id', () => {
    expect(deepLinkPath({ contentType: 'movie', tmdbId: 603 })).toBe('movie/603')
  })

  it('routes series by title slug', () => {
    expect(deepLinkPath({ contentType: 'series', titleSlug: 'the-wire' })).toBe('series/the-wire')
  })

  it('routes artists by MusicBrainz id, url-encoded', () => {
    expect(deepLinkPath({ contentType: 'artist', musicBrainzId: 'abc-123' })).toBe('artist/abc-123')
  })

  it('returns null when the required id is missing', () => {
    // A series synced before the slug column existed carries an empty slug.
    expect(deepLinkPath({ contentType: 'series', titleSlug: '' })).toBeNull()
    expect(deepLinkPath({ contentType: 'movie' })).toBeNull()
    expect(deepLinkPath({ contentType: 'artist', musicBrainzId: '' })).toBeNull()
  })
})

describe('deepLinkHref', () => {
  const instance = (externalUrl: string): InstanceLink => ({
    id: 'x',
    kind: 'radarr',
    externalUrl,
  })

  it('joins the path onto the instance external URL', () => {
    expect(deepLinkHref(instance('https://radarr.example.com/'), { contentType: 'movie', tmdbId: 42 })).toBe(
      'https://radarr.example.com/movie/42',
    )
  })

  it('preserves a base path on the external URL', () => {
    expect(
      deepLinkHref(instance('https://host/radarr/'), { contentType: 'movie', tmdbId: 42 }),
    ).toBe('https://host/radarr/movie/42')
  })

  it('returns null when no path can be built', () => {
    expect(deepLinkHref(instance('https://sonarr.example.com/'), { contentType: 'series', titleSlug: '' })).toBeNull()
  })
})

describe('arrName', () => {
  it('maps kinds to display names', () => {
    expect(arrName('sonarr')).toBe('Sonarr')
    expect(arrName('radarr')).toBe('Radarr')
    expect(arrName('lidarr')).toBe('Lidarr')
  })
})
