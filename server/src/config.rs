/// Number of leading zero hex characters a mined block's hash must have to
/// satisfy Proof-of-Work. Each hex character is 4 bits, so difficulty N
/// means roughly a 1-in-16^N chance of any given nonce succeeding - kept
/// low here so mining stays fast (well under a second) on modest
/// development hardware. No dynamic difficulty adjustment yet.
pub const MINING_DIFFICULTY: usize = 4;
