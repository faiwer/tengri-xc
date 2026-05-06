use twox_hash::XxHash3_64;

/// Hash `bytes` with xxh3-64 and return a lowercase 16-char hex string,
/// suitable for use as an HTTP `ETag` header.
pub fn etag_for(bytes: &[u8]) -> String {
    let h = XxHash3_64::oneshot(bytes);
    format!("{h:016x}")
}
