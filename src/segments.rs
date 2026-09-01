use std::{fs, path::PathBuf};

use crate::{BIN_SAVE_LOC, activity::Activity};
use nalgebra::{Point2, Vector, Vector2};
use rkyv::{Archive, Deserialize, Serialize, access, deserialize, rancor};

//Starting with gaps -- basically split gaps at different intervals as I think categorizing
// turns may be a difficult starting point not worth digging into right at the start
// I do want to add uphills and downhills later
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Segment {
    name: String,
    small_gap: Vec<Gate>, //every 5 readings on ref
    med_gap: Vec<Gate>,   //every 20 readings on ref
    large_gap: Vec<Gate>, //every 60 readings on ref
                          // uphills: Vec<(gate,gate)>,
                          // downhills: Vec<(gate,gate)>,
}

// using gate as a method of capturing when the rider goes through a part on the path.
// a line from left to right pivot show the gate itself and the in/out ref are to be able
// to compare whether the rider is going through the gate in the right direction
// ---------------
// for now I dont think I actually need a inside and outside ref given that
// segments are run in one direction and that gates are crossed in order
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Gate {
    left_pivot: (f32, f32),
    right_pivot: (f32, f32),
    // inside_ref: (f32, f32),
    // outside_ref: (f32, f32),
}

impl Gate {
    fn new(points: [(f32, f32); 3], length: f32) -> Gate {
        todo!("Need to convert to metric then do vector math then convert back to lon,lat");
        //check the data_analytics image under gates (2.1) for what the goal is here
        let point_a = Point2::new(points[0].0, points[0].1);
        let point_b = Point2::new(points[1].0, points[1].1);
        let point_c = Point2::new(points[2].0, points[2].1);

        //vectorize the two lines then grab the normalized perpindicular line
        let vec_ab: Vector2<f32> = point_b - point_a;
        let vec_bc: Vector2<f32> = point_c - point_b;
        let vec_ab_perp: Vector2<f32> = Vector2::new(-vec_ab.y, vec_ab.x);
        let vec_bc_perp: Vector2<f32> = Vector2::new(-vec_bc.y, vec_bc.x);
        let norm_vec_ab_perp = vec_ab_perp.normalize() * length / 2.0;
        let norm_vec_bc_perp = vec_bc_perp.normalize() * length / 2.0;

        //Form those two gates based on the two vectors than take the midpoints
        // to form the new finalized gate
        let ab_gate = (point_a - norm_vec_ab_perp, point_b + norm_vec_ab_perp);
        let bc_gate = (point_b - norm_vec_bc_perp, point_c + norm_vec_bc_perp);
        let full_gate: Vector2<f32> = (ab_gate.1 + (bc_gate.1 - ab_gate.1) / 2.0)
            - (ab_gate.0 + (bc_gate.0 - ab_gate.0) / 2.0);

        //Normalize the full gate and apply the length to it
        let full_norm = full_gate.normalize() * length / 2.0;

        let full_left = point_b - full_norm;
        let full_right = point_b - full_norm;

        Gate {
            left_pivot: (full_left.x, full_right.y),
            right_pivot: (full_right.x, full_right.y),
        }
    }
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

    let mut small_gap: Vec<Gate> = Vec::new(); //every 5 readings on ref
    let mut med_gap: Vec<Gate> = Vec::new(); //every 20 readings on ref
    let mut large_gap: Vec<Gate> = Vec::new(); //every 60 readings on ref

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
        .position(|t| t >= &ref_activity.segments[seg_ref_index].start_time)
        .ok_or("segment start position not found")?;
    let seg_end_ind = ref_activity
        .telemetry
        .timestamps
        .iter()
        .position(|t| t >= &ref_activity.segments[seg_ref_index].end_time)
        .ok_or("Segment end position not found")?;

    let mut three_points: [(f32, f32); 3] = [(420.0, 420.0); 3];
    for i in seg_start_ind..seg_end_ind {
        if i / 5 == (i as f32 / 5.0) as usize && i + 3 <= seg_end_ind {
            for (a, ii) in (i..i + 3).enumerate() {
                let (lon, lat) = (
                    ref_activity.telemetry.longitude[ii],
                    ref_activity.telemetry.latitude[ii],
                );
                if lon.is_some() && lat.is_some() {
                    three_points[a].0 = lon.unwrap();
                    three_points[a].1 = lat.unwrap();
                }
            }

            //If any of the coordinates didn't exist (yielding the initial 420.0) then skip
            if !three_points.iter().any(|(x, y)| *x == 420.0 || *y == 420.0) {
                let gate = Gate::new(three_points);
                continue;
            }
        }
    }
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
