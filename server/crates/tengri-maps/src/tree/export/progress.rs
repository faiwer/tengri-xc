use std::collections::VecDeque;
use std::io::Write;
use std::time::{Duration, Instant};

const PROGRESS_WINDOW: Duration = Duration::from_secs(60);

pub(super) struct ProgressWriter {
    writer: Box<dyn Write + Send>,
    total: usize,
    last_percent: usize,
    samples: VecDeque<ProgressSample>,
}

struct ProgressSample {
    at: Instant,
    done: usize,
}

impl ProgressWriter {
    pub(super) fn new(writer: Box<dyn Write + Send>, total: usize) -> Self {
        Self {
            writer,
            total,
            last_percent: 0,
            samples: VecDeque::new(),
        }
    }

    pub(super) fn update(&mut self, done: usize) {
        if self.total == 0 {
            return;
        }
        let percent = done * 100 / self.total;
        if percent == self.last_percent && done != self.total {
            return;
        }
        self.last_percent = percent;
        let total = self.total;
        let details = self.progress_details(Instant::now(), done);
        let _ = writeln!(self.writer, "{done}/{total} ({percent}%){details}");
        let _ = self.writer.flush();
    }

    pub(super) fn finish(&mut self) {
        self.update(self.total);
    }

    pub(super) fn progress_details(&mut self, now: Instant, done: usize) -> String {
        let Some(rate) = self.record_sample(now, done) else {
            return String::new();
        };
        let remaining = self.total.saturating_sub(done);
        let eta = Duration::from_secs_f64(remaining as f64 / rate);
        format!(
            " {} tiles/s eta {}",
            format_rate(rate),
            format_duration(eta)
        )
    }

    fn record_sample(&mut self, now: Instant, done: usize) -> Option<f64> {
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
    format!("{rate:.0}")
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
