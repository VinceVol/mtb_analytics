use std::{fs, path::PathBuf};

use crate::{BIN_SAVE_LOC, activity::Activity};

struct Segment {
    name: String,
    files_w_seg: PathBuf,
    turns: Vec<(f64, f64)>, //turns marked by (start distance, end distance)
    uphills: Vec<(f64, f64)>,
    downhills: Vec<(f64, f64)>,
}

pub fn list_segments() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut seg_name_list = Vec::new();
    for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
        if let Ok(entry) = entry_res {
            if let Ok(activity) =
                Activity::open_bin(&entry.file_name().into_string().unwrap().replace(".bin", ""))
            {
                for seg in activity.segments {
                    if let Some(seg_name) = seg.name {
                        if !seg_name_list.contains(&seg_name)
                            && !seg_name.replace(" ", "").is_empty()
                        {
                            println!("{}", &seg_name);
                            seg_name_list.push(seg_name);
                        }
                    }
                }
            }
        }
    }
    Ok(seg_name_list)
}
fn seg_exists(name: &str) -> Segment {
    todo!();
}
