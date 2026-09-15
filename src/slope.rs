//using this as an area to work on capturing data related to the slope

use crate::gate::{dist_btwn_points, points_to_utm};

//points (long,lat,alt)
pub fn find_slope<const N: usize>(points: [(f32, f32, u32); N]) -> f32 {
    let coordinates: [(f32, f32); N] = points.map(|(x, y, _)| (x, y));
    let utm_points = points_to_utm(coordinates);
    let mut x_axis = [0.0; N];
    let y_axis = points.map(|(_, _, a)| a as f64);
    let start_point: (f64, f64) = *utm_points.0.first().unwrap();
    for (index, (x, y)) in utm_points.0.iter().enumerate() {
        if index == 0 {
            continue;
        }
        x_axis[index] = dist_btwn_points(start_point, (*x, *y));
    }

    // y = a + bx
    let sum_x: f64 = x_axis.iter().sum();
    let sum_y: f64 = y_axis.iter().sum();
    let sum_xy: f64 = x_axis.iter().zip(y_axis).map(|(&x, y)| x * y).sum();
    let sum_x_2: f64 = x_axis.map(|x| x * x).iter().sum();

    let b = ((N as f64) * sum_xy - sum_x * sum_y) / ((N as f64) * sum_x_2 - sum_x * sum_x);

    return 100.0 * b as f32;
}
