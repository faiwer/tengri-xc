//! How often one account may be mailed a reset link.
//!
//! `users.password_reset_sends` holds the last two send times, `[prev, last]`.
//! Each wait is twice the gap between them, so the delay doubles on its own
//! while a requester keeps coming back the moment we let them — no counter to
//! keep in sync with the timestamps.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

/// Wait after a first send, when there's no gap to double yet.
const WAIT_BASE: Duration = Duration::from_secs(60 * 60);

/// Ceiling on the doubling.
const WAIT_MAX: Duration = Duration::from_secs(24 * 60 * 60);

/// Sends older than this belong to an earlier episode and stop counting.
///
/// Must stay above `2 * WAIT_MAX`, or the top of the ladder collapses. At the
/// ceiling the two sends sit `WAIT_MAX` apart and the next request comes
/// another `WAIT_MAX` later, so `prev` is `2 * WAIT_MAX` old by the time it's
/// read — a tighter horizon would filter it out and drop the wait back to
/// [`WAIT_BASE`].
const LADDER_RESET: Duration = Duration::from_secs(72 * 60 * 60);

/// Sends still on the current ladder, oldest first, at most two. Keeping an
/// older one would feed a months-wide gap into the doubling and cap the wait
/// for someone who isn't hammering at all.
pub fn recent_sends(sends: &[DateTime<Utc>], now: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let horizon = TimeDelta::from_std(LADDER_RESET).expect("LADDER_RESET fits in a TimeDelta");
    let mut recent: Vec<_> = sends
        .iter()
        .copied()
        .filter(|sent| now - *sent < horizon)
        .collect();
    if recent.len() > 2 {
        recent.drain(..recent.len() - 2);
    }
    recent
}

/// When the next send becomes allowed, or `None` if one is allowed right away.
pub fn next_allowed(recent: &[DateTime<Utc>]) -> Option<DateTime<Utc>> {
    let base = TimeDelta::from_std(WAIT_BASE).expect("WAIT_BASE fits in a TimeDelta");
    let max = TimeDelta::from_std(WAIT_MAX).expect("WAIT_MAX fits in a TimeDelta");
    match recent {
        // Nothing on the ladder: send now, and the write starts a fresh one.
        [] => None,
        [only] => Some(*only + base),
        [.., prev, last] => Some(*last + ((*last - *prev) * 2).min(max)),
    }
}

/// The value to store after sending at `now`: the ladder we just measured plus
/// this send, trimmed to two. Appends to the *filtered* list so a stale entry
/// can't linger as `prev` forever.
pub fn push_send(recent: Vec<DateTime<Utc>>, now: DateTime<Utc>) -> Vec<DateTime<Utc>> {
    let mut next = recent;
    next.push(now);
    if next.len() > 2 {
        next.drain(..next.len() - 2);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + hours * 3600, 0).expect("in range")
    }

    #[test]
    fn first_request_goes_out_immediately() {
        assert_eq!(next_allowed(&[]), None);
    }

    #[test]
    fn second_request_waits_the_base() {
        assert_eq!(next_allowed(&[at(0)]), Some(at(1)));
    }

    #[test]
    fn the_wait_doubles_the_previous_gap() {
        assert_eq!(next_allowed(&[at(0), at(1)]), Some(at(3)));
        assert_eq!(next_allowed(&[at(1), at(3)]), Some(at(7)));
        assert_eq!(next_allowed(&[at(3), at(7)]), Some(at(15)));
    }

    #[test]
    fn the_wait_stops_at_a_day() {
        assert_eq!(next_allowed(&[at(0), at(20)]), Some(at(20 + 24)));
    }

    /// A stale `prev` would otherwise make the gap enormous and pin the wait at
    /// the ceiling for someone who last reset months ago.
    #[test]
    fn a_stale_entry_drops_off_the_ladder() {
        let sends = [at(0), at(24 * 30)];
        let recent = recent_sends(&sends, at(24 * 30 + 1));
        assert_eq!(recent, vec![at(24 * 30)]);
        assert_eq!(next_allowed(&recent), Some(at(24 * 30 + 1)));
    }

    #[test]
    fn a_long_quiet_spell_clears_the_ladder() {
        let sends = [at(0), at(1)];
        assert!(recent_sends(&sends, at(24 * 7)).is_empty());
    }

    /// A requester sitting at the ceiling reads a `prev` that is already
    /// `2 * WAIT_MAX` old, so the horizon has to outlast *that*, not just
    /// `WAIT_MAX`. Otherwise `prev` drops off and the wait falls back to the
    /// base, handing the ladder's whole purpose away.
    #[test]
    fn the_ceiling_holds_across_repeats() {
        let sends = [at(0), at(24)];
        let recent = recent_sends(&sends, at(48));
        assert_eq!(recent, vec![at(0), at(24)]);
        assert_eq!(next_allowed(&recent), Some(at(48)));
    }

    #[test]
    fn a_send_keeps_only_the_last_two() {
        assert_eq!(push_send(vec![at(0), at(1)], at(3)), vec![at(1), at(3)]);
        assert_eq!(push_send(Vec::new(), at(0)), vec![at(0)]);
    }
}
