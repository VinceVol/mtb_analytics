use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use crate::{BIN_SAVE_LOC, SEGMENT_LOC, activity::Activity};
use nalgebra::{Point2, Vector, Vector2};
use rkyv::{Archive, Deserialize, Serialize, access, deserialize, rancor};
use utm::{
    lat_lon_to_zone_number, lat_to_zone_letter, to_utm_wgs84, to_utm_wgs84_no_zone,
    wsg84_utm_to_lat_lon,
};

//Starting with gaps -- basically split gaps at different intervals as I think categorizing
// turns may be a difficult starting point not worth digging into right at the start
// I do want to add uphills and downhills later
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Segment {
    name: String,
    small_gap: Vec<Gate>, //every 5 readings on ref
    med_gap: Vec<u64>, //every 20 readings on ref usize(as u64 cuz of archive) points to a gate index in small gap
    large_gap: Vec<u64>, //every 60 readings on ref
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
    //length is in meters
    fn new(points: [(f32, f32); 3], length_m: f64) -> Gate {
        let (utm_points, ref_mat) = points_to_utm(points);
        //check the data_analytics image under gates (2.1) for what the goal is here
        let point_a = Point2::new(utm_points[0].0, utm_points[0].1);
        let point_b = Point2::new(utm_points[1].0, utm_points[1].1);
        let point_c = Point2::new(utm_points[2].0, utm_points[2].1);

        //vectorize the two lines then grab the normalized perpindicular line
        let vec_ab: Vector2<f64> = point_b - point_a;
        let vec_bc: Vector2<f64> = point_c - point_b;
        let vec_ab_perp: Vector2<f64> = Vector2::new(-vec_ab.y, vec_ab.x);
        let vec_bc_perp: Vector2<f64> = Vector2::new(-vec_bc.y, vec_bc.x);
        let norm_vec_ab_perp = vec_ab_perp.normalize() * length_m / 2.0;
        let norm_vec_bc_perp = vec_bc_perp.normalize() * length_m / 2.0;

        //Form those two gates based on the two vectors than take the midpoints
        // to form the new finalized gate
        let ab_gate = (point_a - norm_vec_ab_perp, point_b + norm_vec_ab_perp);
        let bc_gate = (point_b - norm_vec_bc_perp, point_c + norm_vec_bc_perp);
        let full_gate: Vector2<f64> = (ab_gate.1 + (bc_gate.1 - ab_gate.1) / 2.0)
            - (ab_gate.0 + (bc_gate.0 - ab_gate.0) / 2.0);

        //Normalize the full gate and apply the length to it
        let full_norm = full_gate.normalize() * length_m / 2.0;

        let full_left = point_b - full_norm;
        let full_right = point_b - full_norm;

        let left_point = (full_left.x, full_right.y);
        let right_point = (full_right.x, full_right.y);

        //They're both using the same zone and letter because they should be very close to one another
        Gate {
            left_pivot: utm_to_points([left_point], [ref_mat[0]])[0],
            right_pivot: utm_to_points([right_point], [ref_mat[0]])[0],
        }
    }
}

impl Segment {
    fn new(ref_activity: &Activity, seg_name: &str) -> Result<Segment, Box<dyn std::error::Error>> {
        //start with some blank vectors to fill

        let mut small_gap: Vec<Gate> = Vec::new(); //every 5 readings on ref
        let mut med_gap: Vec<u64> = Vec::new(); //every 20 readings on ref
        let mut large_gap: Vec<u64> = Vec::new(); //every 60 readings on ref

        // dbg!(&ref_activity.segments);
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
            .position(|t| {
                if t.is_some() {
                    t.as_ref().unwrap() >= &ref_activity.segments[seg_ref_index].start_time.unwrap()
                } else {
                    false
                }
            })
            .ok_or("segment start position not found")?;
        let seg_end_ind = ref_activity
            .telemetry
            .timestamps
            .iter()
            .position(|t| {
                if t.is_some() {
                    t.as_ref().unwrap()
                        >= &(ref_activity.segments[seg_ref_index].start_time.unwrap()
                            + ref_activity.segments[seg_ref_index].elapsed_time.unwrap() / 1000)
                } else {
                    false
                }
            })
            .ok_or("segment end position not found")?;

        let mut three_points: [(f32, f32); 3] = [(420.0, 420.0); 3];
        for i in seg_start_ind..seg_end_ind {
            if (i as f32 / 5.0) == (i as f32 / 5.0) as usize as f32 && i + 3 <= seg_end_ind {
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
                    let gate = Gate::new(three_points, 4.0);
                    if (i as f32 / 20.0) == (i as f32 / 20.0) as usize as f32 {
                        med_gap.push(small_gap.len() as u64);
                    }
                    if (i as f32 / 60.0) == (i as f32 / 60.0) as usize as f32 {
                        large_gap.push(small_gap.len() as u64);
                    }
                    small_gap.push(gate);
                }
            }
        }
        Ok(Segment {
            name: seg_name.to_string(),
            small_gap,
            med_gap,
            large_gap,
        })
    }
    pub fn check_seg(seg_name: &str) -> Result<Segment, Box<dyn std::error::Error>> {
        for entry_res in fs::read_dir(SEGMENT_LOC)? {
            if entry_res.is_ok_and(|e| {
                &e.file_name().into_string().unwrap().replace(".bin", "") == seg_name
            }) {
                return Segment::open_bin(seg_name);
            }
        }

        //create a vec of all ref_activity file loc with their respective segment elapsed time. grab the one with the longest elapsed time
        let mut file_v_seg = Vec::new();
        for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
            if let Ok(entry) = entry_res {
                if let Ok(activity) = Activity::open_bin(
                    &entry.file_name().into_string().unwrap().replace(".bin", ""),
                ) {
                    //Logic is -- if the segment name matches to the one your looking for and both the t_min_pause == t w pause
                    // basically if the run doesnt contain pauses (avoid where maybe I turned back to grab something)
                    if let Some(ref_segment) = activity.segments.iter().find(|s| {
                        if s.name.as_ref().is_some_and(|ss| ss == seg_name)
                            && s.elapsed_time.is_some()
                            && s.t_min_pause.is_some()
                        {
                            if s.elapsed_time.unwrap() == s.t_min_pause.unwrap() {
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }) {
                        file_v_seg.push((entry.file_name(), ref_segment.elapsed_time.unwrap()));
                    }
                }
            }
        }

        //Longest time first shortest time last -> open that activity
        file_v_seg.sort_by(|(_file_1, el_time_1), (_file_2, el_time_2)| el_time_2.cmp(el_time_1));
        let long_act_nme = file_v_seg
            .first()
            .ok_or("Longest activity vector for finding a reference segment")?
            .0
            .to_str()
            .unwrap();
        let long_act = Activity::open_bin(&long_act_nme.replace(".bin", ""))?;
        let segment = Segment::new(&long_act, seg_name)?;
        segment.save_bin()?;
        return Ok(segment);
    }

    fn save_bin(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<rancor::Error>(self)?;

        let fp = format!("{}{}.bin", SEGMENT_LOC, self.name);
        let mut the_file = std::fs::File::create(fp)?;
        the_file.write_all(&bytes)?;
        Ok(())
    }
    fn open_bin(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let fp = format!("{}{}.bin", SEGMENT_LOC, name);
        let mut file = std::fs::File::open(fp)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        // let archived = access::<ArchivedActivity, rancor::Error>(&bytes)?;
        let archived = rkyv::access::<ArchivedSegment, rancor::Error>(&bytes[..]).unwrap();
        let segment: Segment = deserialize::<Segment, rancor::Error>(archived)?;
        Ok(segment)
    }
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

//Cannot do vector math directily on lon,lat points as the earth isn't flat
// utm should be a unit that works well with vectors
fn points_to_utm<const N: usize>(points: [(f32, f32); N]) -> ([(f64, f64); N], [(u8, char); N]) {
    let mut utm_points: [(f64, f64); N] = [(0.0, 0.0); N];
    let mut zone_num_letter: [(u8, char); N] = [(0, 'a'); N];
    for (i, (lon, lat)) in points.iter().enumerate() {
        let (northing, easting, _) = to_utm_wgs84_no_zone(*lat as f64, *lon as f64);
        let zone = lat_lon_to_zone_number(*lat as f64, *lon as f64);
        let letter = lat_to_zone_letter(*lat as f64).unwrap_or('N');

        zone_num_letter[i] = (zone, letter);
        utm_points[i] = (easting, northing);
    }

    (utm_points, zone_num_letter)
}

fn utm_to_points<const N: usize>(
    utm_points: [(f64, f64); N],
    zone_num_letter: [(u8, char); N],
) -> [(f32, f32); N] {
    let mut points: [(f32, f32); N] = [(0.0, 0.0); N];
    for (i, (easting, northing)) in utm_points.iter().enumerate() {
        let (lat, lon) = wsg84_utm_to_lat_lon(
            *easting,
            *northing,
            zone_num_letter[i].0,
            zone_num_letter[i].1,
        )
        .unwrap();

        points[i] = (lon as f32, lat as f32);
    }

    points
}
