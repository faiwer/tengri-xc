use rand::Rng;

/// 8-char NanoID with the `[A-Za-z0-9_-]` alphabet declared in the schema
/// comment. 64 symbols × 8 chars = 48 bits of entropy, ample for the expected
/// row count and matching the spec exactly.
pub fn nanoid_8() -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let mut rng = rand::rng();
    (0..8)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}
