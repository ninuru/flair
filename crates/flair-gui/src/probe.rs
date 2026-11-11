use std::time::{Duration, Instant};

const FRAMES: usize = 120;

pub struct Probe {
    start: Instant,
    marks: Vec<(&'static str, f64)>,
    frames: Option<Vec<f64>>,
}
impl Probe {
    pub fn start(start: Instant) -> Self {
        Self {
            start,
            marks: vec![],
            frames: None,
        }
    }
    pub fn mark(&mut self, phase: &'static str) {
        self.marks
            .push((phase, self.start.elapsed().as_secs_f64() * 1000.0));
    }
    pub fn frame(&mut self, cost: Duration) -> bool {
        match &mut self.frames {
            Some(frames) => {
                frames.push(cost.as_secs_f64() * 1000.0);
                frames.len() >= FRAMES
            }
            None => {
                self.mark("first_frame");
                self.frames = Some(Vec::with_capacity(FRAMES));
                false
            }
        }
    }
    pub fn finish(self) {
        for (phase, elapsed) in &self.marks {
            eprintln!("probe {phase} {elapsed:.1}");
        }
        if let Some((_, total)) = self.marks.last() {
            eprintln!("probe total {total:.1}");
        }
        let mut frames = self.frames.unwrap_or_default();
        frames.sort_by(f64::total_cmp);
        if let (Some(median), Some(max)) = (frames.get(frames.len() / 2), frames.last()) {
            eprintln!("probe frame-median {median:.3}");
            eprintln!("probe frame-max {max:.3}");
        }
    }
}
