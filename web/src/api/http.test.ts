import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError, request } from './http'

function stubFetch(response: Partial<Response> & { jsonImpl?: () => Promise<unknown> }) {
  const fetchMock = vi.fn().mockResolvedValue({
    status: response.status,
    ok: response.ok ?? false,
    json: response.jsonImpl ?? (() => Promise.resolve(undefined)),
  } as unknown as Response)
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

afterEach(() => vi.unstubAllGlobals())

describe('request', () => {
  it('returns null on 204', async () => {
    stubFetch({ status: 204, ok: true })
    expect(await request('/api/v1/sync')).toBeNull()
  })

  it('parses JSON on 2xx', async () => {
    stubFetch({ status: 200, ok: true, jsonImpl: () => Promise.resolve([{ id: 1 }]) })
    expect(await request('/api/v1/content')).toEqual([{ id: 1 }])
  })

  it('throws ApiError with the parsed message on error responses', async () => {
    stubFetch({ status: 500, jsonImpl: () => Promise.resolve({ error: 'internal server error' }) })
    await expect(request('/api/v1/content')).rejects.toMatchObject({
      status: 500,
      message: 'internal server error',
    })
  })

  it('throws ApiError without a parse failure on a body-less 404', async () => {
    stubFetch({ status: 404, jsonImpl: () => Promise.reject(new Error('no body')) })
    const error = await request('/api/v1/playback/sync').catch((caught) => caught)
    expect(error).toBeInstanceOf(ApiError)
    expect((error as ApiError).status).toBe(404)
  })
})
