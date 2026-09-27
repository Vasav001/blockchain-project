/** Shortens a long hex string for compact display, e.g. `abcd1234…ef567890`. */
export function shortHex(value: string, head = 8, tail = 8): string {
  if (value.length <= head + tail + 1) return value
  return `${value.slice(0, head)}…${value.slice(-tail)}`
}

export function formatTimestamp(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString()
}
