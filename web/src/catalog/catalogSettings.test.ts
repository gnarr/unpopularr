import { describe, expect, it } from 'vitest'
import {
  DEFAULT_SETTINGS,
  parseCatalogSettings,
  serializeCatalogSettings,
  type CatalogSettings,
} from './catalogSettings'

describe('catalogSettings', () => {
  it('round-trips a fully populated settings object', () => {
    const settings: CatalogSettings = {
      search: 'matrix',
      types: ['movie', 'series'],
      instanceIds: ['radarr-hd'],
      neverPlayedOnly: true,
      sorting: [{ id: 'plays', desc: false }],
    }
    expect(parseCatalogSettings(serializeCatalogSettings(settings))).toEqual(settings)
  })

  it('returns defaults for null (no stored value)', () => {
    expect(parseCatalogSettings(null)).toEqual(DEFAULT_SETTINGS)
  })

  it('returns defaults for invalid JSON', () => {
    expect(parseCatalogSettings('{')).toEqual(DEFAULT_SETTINGS)
  })

  it('returns defaults for a non-object payload', () => {
    expect(parseCatalogSettings('"a string"')).toEqual(DEFAULT_SETTINGS)
  })

  it('drops unknown content types but keeps valid ones', () => {
    const raw = JSON.stringify({ types: ['movie', 'bogus'] })
    expect(parseCatalogSettings(raw).types).toEqual(['movie'])
  })

  it('preserves an explicitly empty types selection', () => {
    const raw = JSON.stringify({ types: [] })
    expect(parseCatalogSettings(raw).types).toEqual([])
  })

  it('drops sorting entries with unknown column ids, keeps valid ones', () => {
    const raw = JSON.stringify({
      sorting: [
        { id: 'nope', desc: true },
        { id: 'displayName', desc: false },
      ],
    })
    expect(parseCatalogSettings(raw).sorting).toEqual([{ id: 'displayName', desc: false }])
  })

  it('preserves an explicitly empty sorting state (no sort)', () => {
    const raw = JSON.stringify({ sorting: [] })
    expect(parseCatalogSettings(raw).sorting).toEqual([])
  })

  it('falls back per field without discarding valid siblings', () => {
    const raw = JSON.stringify({
      search: 123,
      types: ['artist'],
      instanceIds: {},
      neverPlayedOnly: 'yes',
      sorting: [{ id: 'sizeOnDiskBytes', desc: true }],
    })
    expect(parseCatalogSettings(raw)).toEqual({
      search: '',
      types: ['artist'],
      instanceIds: [],
      neverPlayedOnly: false,
      sorting: [{ id: 'sizeOnDiskBytes', desc: true }],
    })
  })

  it('keeps only string instance ids', () => {
    const raw = JSON.stringify({ instanceIds: ['radarr-hd', 42, null] })
    expect(parseCatalogSettings(raw).instanceIds).toEqual(['radarr-hd'])
  })
})
