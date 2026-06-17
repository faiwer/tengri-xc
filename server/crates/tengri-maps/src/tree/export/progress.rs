use std::collections::VecDeque;
use std::io::Write;
use std::time::{Duration, Instant};

/// Throttle progress output to one line per this much wall time so the
/// log stays readable on long runs without going silent for minutes
/// during a slow integer-percent step.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(5);
/// Rolling window used for the rate / ETA estimate.
const PROGRESS_WINDOW: Duration = Duration::from_secs(60);

pub(super) struct ProgressWriter {
    writer: Box<dyn Write + Send>,
    total: usize,
    started_at: Instant,
    last_emit: Option<Instant>,
    samples: VecDeque<ProgressSample>,
}

struct ProgressSample {
    at: Instant,
    done: usize,
}

impl ProgressWriter {
    pub(super) fn new(writer: Box<dyn Write + Send>, total: usize) -> Self {
        Self::with_start(writer, total, Instant::now())
    }

    pub(super) fn with_start(
        writer: Box<dyn Write + Send>,
        total: usize,
        started_at: Instant,
    ) -> Self {
        Self {
            writer,
            total,
            started_at,
            last_emit: None,
            samples: VecDeque::new(),
        }
    }

    pub(super) fn update(&mut self, done: usize) {
        self.update_at(Instant::now(), done);
    }

    /// Test-friendly entry: takes the wall clock as an argument so unit
    /// tests can drive the throttle deterministically.
    pub(super) fn update_at(&mut self, now: Instant, done: usize) {
        if self.total == 0 {
            return;
        }
        let is_final = done >= self.total;
        let due = match self.last_emit {
            None => done > 0,
            Some(at) => now.duration_since(at) >= PROGRESS_INTERVAL,
        };
        if !due && !is_final {
            return;
        }
        self.last_emit = Some(now);
        let percent = done * 100 / self.total;
        let total = self.total;
        let details = self.progress_details(now, done);
        let _ = writeln!(self.writer, "{done}/{total} ({percent}%){details}");
        let _ = self.writer.flush();
    }

    pub(super) fn finish(&mut self) {
        self.update(self.total);
    }

    pub(super) fn progress_details(&mut self, now: Instant, done: usize) -> String {
        // Window rate first — it tracks current throughput. Fall back to "rate
        // since start" so the very first emitted line still carries an ETA
        // instead of going blank.
        let rate = match self.window_rate(now, done) {
            Some(rate) => rate,
            None => match self.overall_rate(now, done) {
                Some(rate) => rate,
                None => return String::new(),
            },
        };
        let remaining = self.total.saturating_sub(done);
        let eta = Duration::from_secs_f64(remaining as f64 / rate);
        format!(
            " {} blocks/s eta {}",
            format_rate(rate),
            format_duration(eta)
        )
    }

    fn window_rate(&mut self, now: Instant, done: usize) -> Option<f64> {
        self.samples.push_back(ProgressSample { at: now, done });
        while self.samples.len() > 1
            && now.duration_since(self.samples.front()?.at) > PROGRESS_WINDOW
        {
            self.samples.pop_front();
        }

        let first = self.samples.front()?;
        let last = self.samples.back()?;
        let elapsed = last.at.duration_since(first.at).as_secs_f64();
        let delta = last.done.saturating_sub(first.done);
        if elapsed == 0.0 || delta == 0 {
            return None;
        }
        Some(delta as f64 / elapsed)
    }

    fn overall_rate(&self, now: Instant, done: usize) -> Option<f64> {
        let elapsed = now.duration_since(self.started_at).as_secs_f64();
        if elapsed == 0.0 || done == 0 {
            return None;
        }
        Some(done as f64 / elapsed)
    }
}

pub(super) fn update_progress(progress: &mut Option<ProgressWriter>, done: usize) {
    if let Some(progress) = progress.as_mut() {
        progress.update(done);
    }
}

fn format_rate(rate: f64) -> String {
    if rate >= 1_000.0 {
        return format!("{:.1}k", rate / 1_000.0);
    }
    format!("{rate:.2}")
}

pub(super) fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let minutes = secs / 60;
    if minutes < 60 {
        return format!("{}m{}s", minutes, secs % 60);
    }
    format!("{}h{}m", minutes / 60, minutes % 60)
}
