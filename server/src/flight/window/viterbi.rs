//! Two-state Viterbi decoder, log-space.
//!
//! Generic over the meaning of states / emissions: this module is a plain
//! HMM utility and knows nothing about flight detection. The flight pass
//! lives in [`super::detect`].
//!
//! Log-space prevents underflow on long tracks: a 5-hour 1 Hz flight is
//! ~18000 fixes, and naive products of probabilities like 0.9995^18000
//! stay representable, but compounding through the emission probs would
//! get within an order of magnitude of `f64::MIN_POSITIVE`. Working in
//! `ln` makes the whole thing additive and bulletproof.
//!
//! The decoder returns the maximum-a-posteriori state path. Backpointers
//! are stored compactly as `u8` (only two states), which keeps memory at
//! ~17 bytes per fix.

/// Parameters for a 2-state HMM.
///
/// `init[s]` — prior probability of starting in state `s`.
/// `transition[from][to]` — probability of moving from `from` to `to`.
/// `emission[state][symbol]` — probability of emitting `symbol` while in
/// `state`. Symbols are `0` or `1`.
///
/// All values are plain probabilities in `[0, 1]`; the decoder takes their
/// `ln` internally. Rows must sum to 1 (we don't validate; caller's job).
#[derive(Debug, Clone, Copy)]
pub struct HmmParams {
    pub init: [f64; 2],
    pub transition: [[f64; 2]; 2],
    pub emission: [[f64; 2]; 2],
}

/// Decode the most likely state path for the given binary emission stream.
///
/// Returns a `Vec<u8>` of the same length as `emissions`, with each byte
/// either `0` or `1`. An empty input yields an empty output.
pub fn decode(emissions: &[u8], p: &HmmParams) -> Vec<u8> {
    let n = emissions.len();
    if n == 0 {
        return Vec::new();
    }

    let init = [p.init[0].ln(), p.init[1].ln()];
    let trans = [
        [p.transition[0][0].ln(), p.transition[0][1].ln()],
        [p.transition[1][0].ln(), p.transition[1][1].ln()],
    ];
    let emit = [
        [p.emission[0][0].ln(), p.emission[0][1].ln()],
        [p.emission[1][0].ln(), p.emission[1][1].ln()],
    ];

    let mut dp = vec![[f64::NEG_INFINITY; 2]; n];
    let mut bp = vec![[0u8; 2]; n];

    let e0 = sym(emissions[0]);
    dp[0][0] = init[0] + emit[0][e0];
    dp[0][1] = init[1] + emit[1][e0];

    for t in 1..n {
        let e = sym(emissions[t]);
        for s in 0..2 {
            let from0 = dp[t - 1][0] + trans[0][s];
            let from1 = dp[t - 1][1] + trans[1][s];
            let (best, prev) = if from0 >= from1 {
                (from0, 0u8)
            } else {
                (from1, 1u8)
            };
            dp[t][s] = best + emit[s][e];
            bp[t][s] = prev;
        }
    }

    let mut path = vec![0u8; n];
    path[n - 1] = if dp[n - 1][1] >= dp[n - 1][0] { 1 } else { 0 };
    for t in (0..n - 1).rev() {
        path[t] = bp[t + 1][path[t + 1] as usize];
    }
    path
}

fn sym(b: u8) -> usize {
    if b == 0 { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flight_params() -> HmmParams {
        HmmParams {
            init: [0.80, 0.20],
            transition: [[0.9995, 0.0005], [0.0005, 0.9995]],
            emission: [[0.8, 0.2], [0.2, 0.8]],
        }
    }

    #[test]
    fn empty_input_yields_empty_output() {
        let path = decode(&[], &flight_params());
        assert!(path.is_empty());
    }

    /// All-zero emissions: with init heavily biased toward state 0 and
    /// emissions that match, the MAP path is trivially all zeros.
    #[test]
    fn all_zeros_stay_zero() {
        let emissions = vec![0u8; 100];
        let path = decode(&emissions, &flight_params());
        assert!(path.iter().all(|&s| s == 0));
    }

    /// All-one emissions: even though init prefers 0, sticky transitions
    /// plus emissions that match state 1 quickly tip the entire path to 1.
    #[test]
    fn all_ones_settle_to_one() {
        let emissions = vec![1u8; 100];
        let path = decode(&emissions, &flight_params());
        assert!(path.iter().all(|&s| s == 1));
    }

    /// A single `1` outlier in a sea of zeros must be smoothed out: the
    /// transition cost (0.0005) dwarfs the single noisy emission gain.
    #[test]
    fn single_outlier_one_is_smoothed() {
        let mut emissions = vec![0u8; 200];
        emissions[100] = 1;
        let path = decode(&emissions, &flight_params());
        assert!(path.iter().all(|&s| s == 0));
    }

    /// A single `0` outlier in a sea of ones is similarly smoothed.
    #[test]
    fn single_outlier_zero_is_smoothed() {
        let mut emissions = vec![1u8; 200];
        emissions[100] = 0;
        let path = decode(&emissions, &flight_params());
        // Edges should already be in state 1 (sticky transitions); the
        // middle outlier must not drag the path back to 0.
        assert!(path.iter().all(|&s| s == 1));
    }

    /// A clean 0→1 step (e.g. the takeoff transition) flips the path
    /// after a small lead-in: there is no way the "stay in 0" hypothesis
    /// remains MAP once we've seen ~10+ consecutive 1s.
    #[test]
    fn step_zero_to_one_flips_state() {
        let mut emissions = vec![0u8; 50];
        emissions.extend(vec![1u8; 50]);
        let path = decode(&emissions, &flight_params());
        assert_eq!(path[0], 0, "starts in 0");
        assert_eq!(path[99], 1, "ends in 1");
        let switches: usize = path.windows(2).filter(|w| w[0] != w[1]).count();
        assert_eq!(switches, 1, "exactly one state transition");
    }
}
