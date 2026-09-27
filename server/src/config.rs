/// Number of leading zero hex characters a mined block's hash must have to
/// satisfy Proof-of-Work. Each hex character is 4 bits, so difficulty N
/// means roughly a 1-in-16^N chance of any given nonce succeeding - kept
/// low here so mining stays fast (well under a second) on modest
/// development hardware. No dynamic difficulty adjustment yet.
pub const MINING_DIFFICULTY: usize = 4;

/// Reads `HOST` if set, otherwise `127.0.0.1` - a safe default for local
/// development (not reachable from outside the machine). Docker Compose
/// sets this to `0.0.0.0` so the container's published port is actually
/// reachable from the host (see docker-compose.yml) - binding a container
/// process to `127.0.0.1` would make it unreachable from outside the
/// container's own network namespace.
pub fn host() -> String {
    std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Reads `PORT` if set to a valid port number, otherwise `8080`.
pub fn port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080)
}
