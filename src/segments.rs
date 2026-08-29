use std::path::PathBuf;

struct Segment {
    name: String,
    files_w_seg: PathBuf,
    turns: Vec<(f64, f64)>, //turns marked by (start distance, end distance)
    uphills: Vec<(f64, f64)>,
    downhills: Vec<(f64, f64)>,
}

fn seg_exists(name: &str) -> Segment {
    todo!();
}
