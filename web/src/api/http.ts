export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

// Returns parsed JSON for 2xx, `null` for 204, and throws ApiError otherwise.
// The `{ error }` envelope is parsed defensively because some responses (e.g. an
// unmounted route's 404) have no JSON body.
export async function request<T>(path: string, init?: RequestInit): Promise<T | null> {
  const headers = new Headers(init?.headers)
  headers.set('Accept', 'application/json')

  const response = await fetch(path, {
    ...init,
    headers,
  })

  if (response.status === 204) return null
  if (response.ok) return (await response.json()) as T

  let message = `request failed (${response.status})`
  try {
    const body = (await response.json()) as { error?: unknown }
    if (typeof body?.error === 'string') message = body.error
  } catch {
    // No JSON body — keep the status-based message.
  }
  throw new ApiError(response.status, message)
}
