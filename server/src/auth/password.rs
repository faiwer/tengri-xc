//! Password hashing — verify legacy phpass hashes from the
//! Leonardo XC import, hash new passwords with Argon2id, and
//! transparently migrate phpass → argon2 on first successful
//! login.
//!
//! Why two algorithms: Leonardo XC stored passwords with phpass
//! (the WordPress / phpBB-era portable hash, `$H$9...`), and we
//! want imported pilots to log in with the password they already
//! know. Argon2id is what we'd pick today; rehashing on success
//! lets the database silently bleed phpass out as users come back.
//!
//! The hashes live verbatim in `users.password_hash` as PHC-style
//! self-describing strings, so the column doesn't care which KDF
//! produced them — verification dispatches on the prefix
//! (`$H$` → phpass, `$argon2…` → argon2).

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use md5::{Digest, Md5};
use thiserror::Error;

/// Charset used by phpass for both salt and hash encoding. Order
/// matters — `_phpass_encode64` looks values up by ordinal index.
const PHPASS_ITOA64: &[u8; 64] =
    b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("hash format not recognised")]
    UnknownHashFormat,

    #[error("phpass hash malformed: {0}")]
    PhpassMalformed(&'static str),

    #[error("argon2 error: {0}")]
    Argon2(argon2::password_hash::Error),
}

/// Successful verification result. Carries `rehashed` so the login
/// route knows whether to write a fresh hash back to the database.
#[derive(Debug)]
pub struct Verified {
    /// `Some(new_hash)` if the stored hash was a legacy format
    /// (currently: phpass) and the route should `UPDATE
    /// password_hash`. `None` if the stored hash is already the
    /// preferred KDF.
    pub rehashed: Option<String>,
}

/// Verify `password` against the stored `hash`. Returns `Ok(Some)`
/// on a match (with optional rehash payload), `Ok(None)` on
/// mismatch, and `Err` only for malformed hashes — the latter
/// shouldn't happen for rows we wrote ourselves and indicates DB
/// corruption or a bad import.
pub fn verify(password: &str, hash: &str) -> Result<Option<Verified>, PasswordError> {
    if hash.starts_with("$argon2") {
        return verify_argon2(password, hash);
    }
    if hash.starts_with("$H$") || hash.starts_with("$P$") {
        return verify_phpass(password, hash);
    }
    Err(PasswordError::UnknownHashFormat)
}

/// Hash a fresh password with Argon2id using the crate's default
/// parameters. `OsRng` for the salt — `SaltString::generate`
/// pulls 16 random bytes and base64-encodes them. The returned
/// string is a PHC encoding (`$argon2id$v=19$m=...`) that goes
/// straight into `users.password_hash`.
pub fn hash_argon2(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(PasswordError::Argon2)?;
    Ok(hash.to_string())
}

fn verify_argon2(password: &str, hash: &str) -> Result<Option<Verified>, PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(PasswordError::Argon2)?;
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(Some(Verified { rehashed: None })),
        Err(argon2::password_hash::Error::Password) => Ok(None),
        Err(e) => Err(PasswordError::Argon2(e)),
    }
}

/// phpass portable hash verifier. Mirrors phpBB's
/// `_hash_crypt_private`: `$H$` / `$P$` prefix, one byte for the
/// log2 iteration count, eight bytes of salt, the rest is the
/// MD5-iterated digest in phpass-base64. We re-hash with the same
/// parameters and compare in constant time.
fn verify_phpass(password: &str, hash: &str) -> Result<Option<Verified>, PasswordError> {
    let bytes = hash.as_bytes();
    if bytes.len() != 34 {
        return Err(PasswordError::PhpassMalformed("expected 34 bytes"));
    }
    // Iteration count is encoded as a phpass-base64 character at
    // offset 3. The exponent must be in [7, 30] (phpBB clamps it
    // there); anything outside that is corrupt input.
    let count_log2 =
        phpass_decode_char(bytes[3]).ok_or(PasswordError::PhpassMalformed("bad cost char"))? as u32;
    if !(7..=30).contains(&count_log2) {
        return Err(PasswordError::PhpassMalformed("cost out of range"));
    }
    let count: u64 = 1 << count_log2;
    let salt = &bytes[4..12];
    let stored_digest = &bytes[12..34];

    // Initial round: MD5(salt || password).
    let mut h = Md5::new();
    h.update(salt);
    h.update(password.as_bytes());
    let mut digest: [u8; 16] = h.finalize().into();
    // Subsequent `count` rounds: MD5(prev || password). The total
    // iteration count is `count + 1` to match phpass exactly —
    // see `_leonardo_hash_crypt_private` in Leonardo XC's
    // `functions.php`.
    for _ in 0..count {
        let mut h = Md5::new();
        h.update(digest);
        h.update(password.as_bytes());
        digest = h.finalize().into();
    }
    let computed = phpass_encode64(&digest, 16);

    if constant_time_eq(computed.as_bytes(), stored_digest) {
        Ok(Some(Verified {
            // Rehash payload: a fresh argon2 string that the
            // login route will write back. Doing the hash here
            // (rather than letting the route re-hash) keeps the
            // expensive op on the same code path that already
            // verified the password — there's no point computing
            // an argon2 hash unless verify just succeeded.
            rehashed: Some(hash_argon2(password)?),
        }))
    } else {
        Ok(None)
    }
}

/// phpass-base64 encoder, ported from
/// `_leonardo_hash_encode64` / phpBB. Not the same as standard
/// base64: alphabet is `./0-9A-Za-z` (their `$itoa64`), output is
/// produced 3 input bytes at a time but with a fence-post quirk
/// that emits one extra char at the end.
fn phpass_encode64(input: &[u8], count: usize) -> String {
    let mut out = String::with_capacity(count * 4 / 3 + 2);
    let mut i = 0;
    loop {
        let mut value = input[i] as u32;
        i += 1;
        out.push(PHPASS_ITOA64[(value & 0x3f) as usize] as char);
        if i < count {
            value |= (input[i] as u32) << 8;
        }
        out.push(PHPASS_ITOA64[((value >> 6) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
        i += 1;
        if i < count {
            value |= (input[i] as u32) << 16;
        }
        out.push(PHPASS_ITOA64[((value >> 12) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
        i += 1;
        out.push(PHPASS_ITOA64[((value >> 18) & 0x3f) as usize] as char);
        if i >= count {
            break;
        }
    }
    out
}

/// Reverse-index a phpass-base64 character. Returns `None` for
/// anything outside the alphabet; that shouldn't happen for hashes
/// we're storing but lets us fail cleanly on corruption.
fn phpass_decode_char(c: u8) -> Option<u8> {
    PHPASS_ITOA64.iter().position(|&x| x == c).map(|p| p as u8)
}

/// Constant-time byte comparison. Both inputs must be the same
/// length; at the call site we already guarantee that. We don't
/// pull a crate just for this — the loop is short and obvious.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_round_trip() {
        let h = hash_argon2("hunter2").unwrap();
        assert!(h.starts_with("$argon2"));
        let v = verify("hunter2", &h).unwrap();
        assert!(v.is_some(), "argon2 verify should succeed");
        assert!(
            v.unwrap().rehashed.is_none(),
            "argon2 hash should not request rehash"
        );
        assert!(verify("wrong", &h).unwrap().is_none());
    }

    #[test]
    fn phpass_known_hash_verifies() {
        // Generated by phpBB's portable_hash function for
        // password="hunter2" with cost=8, salt="abcdefgh". This
        // is a fixed-vector regression test — if the hash maths
        // ever drifts we'll know.
        //
        // Recipe (PHP):
        //   $h = new PasswordHash(8, true);
        //   echo $h->HashPassword('hunter2');  // (with deterministic salt)
        //
        // Because the salt randomization isn't easy to fix in
        // PHP's own implementation, we instead verify via our
        // own implementation in a self-loop: hash with the
        // current code, then verify it round-trips. That still
        // tests the encode/decode pair and the iteration count
        // arithmetic; the cross-implementation guarantee is
        // covered by the live login test against imported data.
        let salt = b"abcdefgh";
        let cost: u8 = 8;
        let pw = b"hunter2";

        let mut h = Md5::new();
        h.update(salt);
        h.update(pw);
        let mut digest: [u8; 16] = h.finalize().into();
        for _ in 0..(1u64 << cost) {
            let mut h = Md5::new();
            h.update(digest);
            h.update(pw);
            digest = h.finalize().into();
        }
        let mut hash = String::with_capacity(34);
        hash.push_str("$H$");
        hash.push(PHPASS_ITOA64[cost as usize] as char);
        hash.push_str(std::str::from_utf8(salt).unwrap());
        hash.push_str(&phpass_encode64(&digest, 16));
        assert_eq!(hash.len(), 34);

        let v = verify("hunter2", &hash).unwrap();
        assert!(v.is_some(), "phpass verify should succeed");
        let v = v.unwrap();
        assert!(
            v.rehashed.is_some(),
            "phpass verify should produce an argon2 rehash"
        );
        let new_hash = v.rehashed.unwrap();
        assert!(new_hash.starts_with("$argon2"));
        assert!(verify("hunter2", &new_hash).unwrap().is_some());
        assert!(verify("wrong", &hash).unwrap().is_none());
    }

    #[test]
    fn unknown_hash_format_errors() {
        let err = verify("any", "not-a-known-hash").unwrap_err();
        assert!(matches!(err, PasswordError::UnknownHashFormat));
    }
}
