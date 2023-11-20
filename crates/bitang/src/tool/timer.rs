use std::time::Instant;

pub struct Timer {
    instant: Option<Instant>,
    start: f32,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            instant: None,
            start: 0.0,
        }
    }

    pub fn now(&self) -> f32 {
        self.start
            + self
                .instant
                .map_or(0.0, |instant| instant.elapsed().as_secs_f32())
    }

    pub fn start(&mut self) {
        if self.instant.is_none() {
            self.instant = Some(Instant::now());
        }
    }

    pub fn pause(&mut self) {
        if let Some(instant) = self.instant {
            self.start += instant.elapsed().as_secs_f32();
            self.instant = None;
        }
    }

    pub fn set(&mut self, time: f32) {
        self.start = time;
        if self.instant.is_some() {
            self.instant = Some(Instant::now());
        }
    }

    pub fn is_playing(&self) -> bool {
        self.instant.is_some()
    }
}
