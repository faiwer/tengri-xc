//! In-place "[i / total] label" progress line on stderr. Tiny by
//! design — anything fancier (multi-bar, ETA, spinner) earns a dep
//! like `indicatif`. The migrator's loop is fast enough that one
//! counter line is all the operator needs.
//!
//! Why stderr: stdout stays clean for piping (`leonardo migrate >
//! report.txt` should give you the orchestrator's summary, not
//! 1230 progress lines). Why ANSI escape codes by hand: there's
//! exactly one — `\x1b[K` (erase to end of line) — and pulling in
//! `crossterm` for it would be silly.
//!
//! The renderer is a no-op when stderr isn't a TTY (CI, file
//! redirection, the test harness): the `\r`-overwrite trick produces
//! one giant line otherwise.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

/// Minimum gap between renders. 30 fps is more than enough for a
/// progress counter; rendering on every row at the migrator's rate
/// (~hundreds of rows/s on cached re-runs) just burns terminal IO
/// and makes stderr flicker.
const REDRAW_INTERVAL: Duration = Duration::from_millis(33);

pub struct Progress {
    total: usize,
    done: usize,
    label: &'static str,
    /// `None` if stderr isn't a TTY (or we couldn't tell). All
    /// methods become no-ops in that mode so the migrator runs
    /// identically under `tee`, CI logs, or `cargo test` capture.
    enabled: bool,
    /// Tracks the last render so we can throttle. Initialised to a
    /// time far in the past so the first row always renders.
    last_render: Instant,
}

impl Progress {
    pub fn new(label: &'static str, total: usize) -> Self {
        Self {
            total,
            done: 0,
            label,
            enabled: std::io::stderr().is_terminal(),
            last_render: Instant::now() - REDRAW_INTERVAL,
        }
    }

    /// Bump the counter and (maybe) redraw. Force-renders the final
    /// tick (`done == total`) regardless of throttling so the line
    /// always lands at `[N/N]` before [`finish`].
    pub fn tick(&mut self) {
        self.done += 1;
        if !self.enabled {
            return;
        }
        let is_last = self.done == self.total;
        if !is_last && self.last_render.elapsed() < REDRAW_INTERVAL {
            return;
        }
        self.render();
        self.last_render = Instant::now();
    }

    /// Wipe the progress line so subsequent output (the orchestrator's
    /// summary, errors) doesn't land next to a half-overwritten
    /// counter. Idempotent and safe to call when disabled.
    pub fn finish(&self) {
        if !self.enabled {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r\x1b[K");
        let _ = err.flush();
    }

    fn render(&self) {
        let mut err = std::io::stderr().lock();
        // `\r` → return cursor to column 0; `\x1b[K` → clear from
        // cursor to end of line, so a shorter new line cleanly
        // overwrites a longer previous one without padding.
        let _ = write!(
            err,
            "\r\x1b[K[{} / {}] {}",
            self.done, self.total, self.label
        );
        let _ = err.flush();
    }
}
