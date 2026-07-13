import { describe, expect, it } from 'vitest'
import { arrName, deepLinkHref } from './deepLink'

describe('deepLinkHref', () => {
  it('joins the instance route path onto the external URL', () => {
    expect(deepLinkHref('https://radarr.example.com/', 'movie/inception-27205')).toBe(
      'https://radarr.example.com/movie/inception-27205',
    )
  })

  it('preserves a base path on the external URL', () => {
    expect(deepLinkHref('https://host/radarr/', 'movie/inception-27205')).toBe(
      'https://host/radarr/movie/inception-27205',
    )
  })

  it('returns null when the instance has no route path', () => {
    // A snapshot synced before the slug column existed carries a null path.
    expect(deepLinkHref('https://sonarr.example.com/', null)).toBeNull()
  })
})

describe('arrName', () => {
  it('maps kinds to display names', () => {
    expect(arrName('sonarr')).toBe('Sonarr')
    expect(arrName('radarr')).toBe('Radarr')
    expect(arrName('lidarr')).toBe('Lidarr')
  })
})
