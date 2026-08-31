use std::{fs, path::PathBuf};

use crate::{BIN_SAVE_LOC, activity::Activity};
use rkyv::{Archive, Deserialize, Serialize, access, deserialize, rancor};

#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Segment {
    name: String,
    turns: Vec<((f32, f32), (f32, f32))>, //turns marked by (start long,lat , end long,lat)
    uphills: Vec<((f32, f32), (f32, f32))>, //turns marked by (start long,lat , end long,lat)
    downhills: Vec<((f32, f32), (f32, f32))>, //turns marked by (start long,lat , end long,lat)
}

// using gate as a method of capturing when the rider goes through a part on the path.
// a line from left to right pivot show the gate itself and the in/out ref are to be able
// to compare whether the rider is going through the gate in the right direction
struct gate {
    left_pivot: (f32, f32),
    right_pivot: (f32, f32),
    inside_ref: (f32, f32),
    outside_ref: (f32, f32),
}

impl Segment {
    fn build_seg(seg_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
            if let Ok(entry) = entry_res {
                if let Ok(activity) = Activity::open_bin(
                    &entry.file_name().into_string().unwrap().replace(".bin", ""),
                ) {
                    if activity
                        .segments
                        .iter()
                        .any(|s| s.name.as_ref().is_some_and(|ss| ss == seg_name))
                    {
                        todo!("map segment");
                    }
                }
            }
        }
        Ok(())
    }
}

fn map_segment(ref_activity: &Activity, seg_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    //start with some blank vectors to fill

    let mut turns: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let mut uphills: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let mut downhills: Vec<((f32, f32), (f32, f32))> = Vec::new();

    //choose the first occurence of the segment within the activity
    let seg_ref_index = ref_activity
        .segments
        .iter()
        .position(|s| s.name.as_ref().is_some_and(|n| n == seg_name))
        .unwrap();

    //figure out what part of the data pertains to us
    let seg_start_ind = ref_activity
        .telemetry
        .timestamps
        .iter()
        .position(|t| t >= &ref_activity.segments[seg_ref_index].start_time);
    let seg_end_ind = ref_activity
        .telemetry
        .timestamps
        .iter()
        .position(|t| t >= &ref_activity.segments[seg_ref_index].end_time);

    todo!();
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
