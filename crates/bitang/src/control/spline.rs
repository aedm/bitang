use serde::{Deserialize, Serialize};
use std::cmp::min;

// Plain Catmull-Rom spline ffs
#[derive(Serialize, Deserialize)]
pub struct Spline {
    pub points: Vec<SplinePoint>,
}

#[derive(Serialize, Deserialize)]
pub struct SplinePoint {
    pub time: f32,
    pub value: f32,
    pub is_linear_after: bool,
}

impl Spline {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    fn calculate_tangent(before: &SplinePoint, after: &SplinePoint) -> f32 {
        let dt = after.time - before.time;
        if dt < f32::EPSILON {
            return 0.0;
        }
        (after.value - before.value) / dt
    }

    pub fn get_value(&self, time: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }

        // Unwrap is safe: time is always a valid float
        let res = self
            .points
            .binary_search_by(|p| p.time.partial_cmp(&time).unwrap());

        let index_after = match res {
            Ok(index) => index,
            Err(index) => index,
        };

        if index_after == 0 {
            return self.points[0].value;
        }
        if index_after >= self.points.len() {
            return self.points[self.points.len() - 1].value;
        }

        // https://en.wikipedia.org/wiki/Centripetal_Catmull%E2%80%93Rom_spline
        let p0 = &self.points[index_after.saturating_sub(2)];
        let p1 = &self.points[index_after.saturating_sub(1)];
        let p2 = &self.points[index_after];
        let p3 = &self.points[min(index_after + 1, self.points.len() - 1)];

        let dt = p2.time - p1.time;
        if dt < f32::EPSILON {
            return p1.value;
        }

        let tangent1 = if index_after > 1 { Self::calculate_tangent(p0, p2) } else { 0.0 };
        let tangent2 =
            if index_after < self.points.len() - 1 { Self::calculate_tangent(p1, p3) } else { 0.0 };

        let ft = (time - p1.time) / dt;
        let ea = p1.value;
        let eb = dt * tangent1;
        let ec = 3.0 * (p2.value - p1.value) - dt * (2.0 * tangent1 + tangent2);
        let ed = -2.0 * (p2.value - p1.value) + dt * (tangent1 + tangent2);
        ea + ft * eb + ft * ft * ec + ft * ft * ft * ed
    }
}
