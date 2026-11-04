use crate::geometry;
use monstertruck::modeling::*;

use sims_flow::lattice::Domain;
pub use sims_flow::voxel::*;

pub fn surface(solid: &Solid, spacing: f64) -> (Vec<Point3>, Vec<[usize; 3]>) {
    let tolerance = (spacing * 0.05).min(geometry::mesh_tolerance(solid));
    geometry::welded(&geometry::mesh(solid, tolerance))
}

pub fn place(domain: &mut Domain, grid: &Grid, solid: &Solid, body: u16) -> usize {
    let (points, triangles) = surface(solid, grid.spacing);
    place_surface(domain, grid, &points, &triangles, body)
}
