use glam::Vec2;

struct BezierSpline {
    pub points: Vec<BezierPoint>,
}

struct BezierPoint {
    pub position: Vec2,
    pub in_tangent: Vec2,
    pub out_tangent: Vec2,
}

impl BezierSpline {
    fn new() -> Self {
        Self { points: Vec::new() }
    }

    fn add_point(&mut self, position: Vec2) {
        let in_tangent = Vec2::ZERO;
        let out_tangent = Vec2::ZERO;
        self.points.push(BezierPoint {
            position,
            in_tangent,
            out_tangent,
        });
    }
}
