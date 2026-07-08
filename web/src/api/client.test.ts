import { afterEach, describe, expect, it, vi } from 'vitest'
import { getSeries } from './client'
import { ApiError } from './http'
import type { SeriesDetails } from './types'

function stubFetch(response: { status: number; ok?: boolean; jsonImpl?: () => Promise<unknown> }) {
  const fetchMock = vi.fn().mockResolvedValue({
    status: response.status,
    ok: response.ok ?? false,
    json: response.jsonImpl ?? (() => Promise.resolve(undefined)),
  } as unknown as Response)
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

afterEach(() => vi.unstubAllGlobals())

describe('getSeries', () => {
  it('requests the series endpoint and returns the parsed details', async () => {
    const details: SeriesDetails = {
      displayName: 'The Wire',
      tvdbId: 79126,
      year: 2002,
      sizeOnDiskBytes: 200,
      fileCount: 60,
      instances: [],
      seasons: [
        {
          seasonNumber: 1,
          fileCount: 13,
          episodeCount: 13,
          episodesWithFiles: 13,
          sizeOnDiskBytes: 200,
          playback: null,
          episodes: [],
        },
      ],
      instanceDetails: [],
      playback: null,
      unattributedPlayCount: null,
    }
    const fetchMock = stubFetch({ status: 200, ok: true, jsonImpl: () => Promise.resolve(details) })

    expect(await getSeries(79126)).toEqual(details)
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/v1/series/79126')
  })

  it('rejects with a 404 ApiError for an unknown series', async () => {
    stubFetch({ status: 404, jsonImpl: () => Promise.resolve({ error: 'not found' }) })

    const error = await getSeries(999).catch((caught) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).status).toBe(404)
  })
})
