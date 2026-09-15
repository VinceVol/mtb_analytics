//using this as an area to work on capturing data related to the slope

//points (long,lat,alt)
pub fn find_slope<const N: usize>(points: [(u32, u32); N]) -> f32 {
    let x_axis = points.map(|(d, _)| d as f64);
    let y_axis = points.map(|(_, a)| a as f64);

    // y = a + bx
    let sum_x: f64 = x_axis.iter().sum();
    let sum_y: f64 = y_axis.iter().sum();
    let sum_xy: f64 = x_axis.iter().zip(y_axis).map(|(&x, y)| x * y).sum();
    let sum_x_2: f64 = x_axis.map(|x| x * x).iter().sum();

    //seperate math to make sure it is not zero
    let denom = (N as f64) * sum_x_2 - sum_x * sum_x;
    if denom == 0.0 {
        return 0.0;
    }
    let b = ((N as f64) * sum_xy - sum_x * sum_y) / denom;
    println!(
        "x: {:?}, y: {:?} w slope: b = {}",
        x_axis,
        y_axis,
        b * 100.0
    );

    return 100.0 * b as f32;
}
