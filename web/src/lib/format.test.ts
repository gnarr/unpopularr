import { describe, expect, it } from 'vitest'
import { formatBytes, formatDuration, relativeTime } from './format'

describe('formatBytes', () => {
  it('handles zero and negatives', () => {
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(-5)).toBe('0 B')
  })

  it('uses binary units', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(1024)).toBe('1 KiB')
    expect(formatBytes(1536)).toBe('1.5 KiB')
    expect(formatBytes(1024 ** 3)).toBe('1 GiB')
    expect(formatBytes(1.5 * 1024 ** 4)).toBe('1.5 TiB')
  })
})

describe('formatDuration', () => {
  it('formats sub-minute, minutes, and hours', () => {
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(45)).toBe('45s')
    expect(formatDuration(90)).toBe('1m 30s')
    expect(formatDuration(7200)).toBe('2h 0m')
  })
})

describe('relativeTime', () => {
  const now = new Date('2026-06-25T12:00:00Z').getTime()

  it('formats past and future against a fixed clock', () => {
    expect(relativeTime('2026-06-25T11:00:00Z', now)).toBe('1 hour ago')
    expect(relativeTime('2026-06-24T12:00:00Z', now)).toBe('yesterday')
    expect(relativeTime('2026-06-25T14:00:00Z', now)).toBe('in 2 hours')
  })

  it('returns a dash for invalid input', () => {
    expect(relativeTime('not-a-date', now)).toBe('—')
  })
})
