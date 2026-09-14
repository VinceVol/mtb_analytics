//using this as an area to work on capturing data related to the slope

use crate::gate::points_to_utm;

//points (long,lat,alt)
pub fn find_slope<const N: usize>(points: [(f32, f32, u32); N]) -> f32 {
    let coordinates: [(f32, f32); N] = points.map(|(x, y, _)| (x, y));
    let utm_points = points_to_utm(coordinates);
    let x_axis = [0.0; N];
    for (x, y) in utm_points.0 {
        todo!();
    }

    todo!();
}
