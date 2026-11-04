use monstertruck::modeling::*;

#[derive(Clone, Debug)]
pub struct Exact(pub Surface);

impl sims_tet::Surface for Exact {
    fn at(&self, uv: Vector2) -> Option<Point3> {
        Some(self.0.subs(uv.x, uv.y))
    }

    fn nearest(&self, p: Point3, hint: Option<Vector2>) -> Option<Vector2> {
        let (u, v) = self
            .0
            .search_nearest_parameter(p, hint.map(|h| (h.x, h.y)), 40)?;
        Some(Vector2::new(u, v))
    }
}
