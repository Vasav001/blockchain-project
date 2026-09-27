/**
 * Mirrors `MINING_DIFFICULTY` in server/src/config.rs. There's no API
 * endpoint exposing it (a read-only constant isn't worth a backend change
 * for), so it's duplicated here for display purposes only - it is never
 * used to validate or compute anything client-side.
 */
export const MINING_DIFFICULTY = 4
