use std::time::Instant;
pub struct Probe {
    start: Instant,
    marks: Vec<(&'static str, f64)>,
}
impl Probe {
    pub fn start(start: Instant) -> Self {
        Self {
            start,
            marks: vec![],
        }
    }
    pub fn mark(&mut self, phase: &'static str) {
        self.marks
            .push((phase, self.start.elapsed().as_secs_f64() * 1000.0));
    }
    pub fn finish(self) {
        for (phase, elapsed) in &self.marks {
            eprintln!("probe {phase} {elapsed:.1}");
        }
        if let Some((_, total)) = self.marks.last() {
            eprintln!("probe total {total:.1}");
        }
    }
}
