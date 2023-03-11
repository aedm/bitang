use glam::Vec2;
use std::cmp::{max, min};

// Plain Catmull-Rom spline ffs
pub struct Spline {
    pub points: Vec<SplinePoint>,
}

pub struct SplinePoint {
    pub time: f32,
    pub value: f32,
    pub is_linear_after: bool,
}

impl Spline {
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    pub fn get_value(&self, time: f32) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }

        let res = self.points.binary_search_by_key(&time, |p| p.time);
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
        let p0 = &self.points[max(index_after - 2, 0)];
        let p1 = &self.points[max(index_after - 1, 0)];
        let p2 = &self.points[index_after];
        let p3 = &self.points[min(index_after, self.points.len() - 1)];

        let t0 = p0.time;
        let t1 = p1.time;
        let t2 = p2.time;
        let t3 = p3.time;
        let t = time;

        let a1 = (t1 - t) / (t1 - t0) * p0.value + (t - t0) / (t1 - t0) * p1.value;
        let a2 = (t2 - t) / (t2 - t1) * p1.value + (t - t1) / (t2 - t1) * p2.value;
        let a3 = (t3 - t) / (t3 - t2) * p2.value + (t - t2) / (t3 - t2) * p3.value;
        let b1 = (t2 - t) / (t2 - t0) * a1 + (t - t0) / (t2 - t0) * a2;
        let b2 = (t3 - t) / (t3 - t1) * a2 + (t - t1) / (t3 - t1) * a3;
        let c = (t2 - t) / (t2 - t1) * b1 + (t - t1) / (t2 - t1) * b2;

        //
        // let a1 = (p1.time - time) / (p1.time - p0.time) * p0.value
        //     + (time - p0.time) / (p1.time - p0.time) * p1.value;
        // let a2 = (p2.time - time) / (p2.time - p1.time) * p1.value
        //     + (time - p1.time) / (p2.time - p1.time) * p2.value;
        // let a3 = (p3.time - time) / (p3.time - p2.time) * p2.value
        //     + (time - p2.time) / (p3.time - p2.time) * p3.value;
        // let b1 = (p2.time - time) / (p2.time - p0.time) * a1
        //     + (time - p0.time) / (p2.time - p0.time) * a2;
        // let b2 = (p3.time - time) / (p3.time - p1.time) * a2
        //     + (time - p1.time) / (p3.time - p1.time) * a3;
        // let c = (p2.time - time) / (p2.time - p1.time) * b1
        //     + (time - p1.time) / (p2.time - p1.time) * b2;
        c
    }
}
