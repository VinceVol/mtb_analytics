use chrono::{DateTime, Utc};
use std::{
    fmt, fs,
    io::{Read, Write},
};

use crate::{
    BIN_SAVE_LOC, SEGMENT_LOC,
    activity::{Activity, SegmentRef},
};
use nalgebra::{Point2, RealField, Vector2};
use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};
use utm::{lat_lon_to_zone_number, lat_to_zone_letter, to_utm_wgs84_no_zone, wsg84_utm_to_lat_lon};

//Starting with gaps -- basically split gaps at different intervals as I think categorizing
// turns may be a difficult starting point not worth digging into right at the start
// I do want to add uphills and downhills later
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Segment {
    name: String,
    ref_length: u32,      //meters/100
    small_gap: Vec<Gate>, //every 5 readings on ref
    med_gap: Vec<u64>, //every 20 readings on ref usize(as u64 cuz of archive) points to a gate index in small gap
    large_gap: Vec<u64>, //every 60 readings on ref
    start_end_pos: [(f32, f32); 2], //reference to determine whether the segment was finished
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
            .position(|s| s.name == seg_name)
            .unwrap();

        //Determine the distance covered in the segment
        let ref_length = ref_activity
            .telemetry
            .distance_m
            .iter()
            .find(|t| {
                if t.is_some() {
                    t.as_ref().unwrap() >= &ref_activity.segments[seg_ref_index].distance.unwrap()
                } else {
                    false
                }
            })
            .ok_or("segment distance traveled not found")?
            .ok_or("Segment distance not found")?;

        // copy over the start and end pos
        let start_end_pos = ref_activity.segments[seg_ref_index].start_end_pos;

        //figure out what part of the data pertains to us
        let seg_start_ind = ref_activity
            .telemetry
            .timestamps
            .iter()
            .position(|t| {
                if t.is_some() {
                    t.as_ref().unwrap() >= &ref_activity.segments[seg_ref_index].start_time
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
                        >= &(ref_activity.segments[seg_ref_index].start_time
                            + ref_activity.segments[seg_ref_index].elapsed_time / 1000)
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
            ref_length,
            small_gap,
            med_gap,
            large_gap,
            start_end_pos,
        })
    }

    //need to be able to compare activities to this ref segment and determine whether they include the full run
    // determined earlier that distance isn't a good metric because a speed sensor captures much more data
    // compared to GPS on it's own so distance is not repeatable
    //
    // Rather this function is using the start stop loc and using some linear alegra to determine the distance
    // of those start stop points in the ref to the activity and if that is less than 100 m for both than it should
    // be a pass
    //
    // The reason we need to gatekeep activities is because we don't want our PR to be based on a unfinished segment
    // run (which returns the shortest time) -- honestly wish garmin wouldn't count an incomplete segment
    pub fn start_stop_equal(&self, activity_seg: &SegmentRef) -> bool {
        let utm_ref_ss = points_to_utm(self.start_end_pos).0;
        let utm_act = points_to_utm(activity_seg.start_end_pos).0;

        //within 100m return true
        dist_btwn_points(utm_ref_ss[0], utm_act[0]) < 100.0
            && dist_btwn_points(utm_ref_ss[1], utm_act[1]) < 100.0
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
                    if let Some(ref_segment) = activity
                        .segments
                        .iter()
                        .find(|s| s.name == seg_name && s.elapsed_time == s.t_min_pause)
                    {
                        file_v_seg.push((entry.file_name(), ref_segment.elapsed_time));
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
                    if !seg_name_list.contains(&seg.name) && !seg.name.replace(" ", "").is_empty() {
                        seg_name_list.push(seg.name);
                    }
                }
            }
        }
    }
    Ok(seg_name_list)
}

#[derive(Clone)]
pub struct SegChoice {
    pub file_name: String,
    pub seg_time: u32,
    pub date_ran: u32,
    pub label: String,
}

impl fmt::Display for SegChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

//Go through the activities and list out the ones that have valid data and are somewhat close
// to the reference distance
// return sortable (numbers) and legible (strings) version of (file,seg time, date ran, seg time (legible), date ran(Legible))
pub fn avail_seg_act(name: &str) -> Result<Vec<SegChoice>, Box<dyn std::error::Error>> {
    let segment_ref = Segment::check_seg(name)?;
    let mut fpn_vs_time = Vec::new();

    for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
        if let Ok(entry) = entry_res {
            if let Ok(activity) =
                Activity::open_bin(&entry.file_name().into_string().unwrap().replace(".bin", ""))
            {
                //Logic is -- if the segment name matches to the one your looking for and both the t_min_pause == t w pause
                // basically if the run doesnt contain pauses (avoid where maybe I turned back to grab something)
                if let Some(segment) = activity.segments.iter().find(|s| {
                    if s.name == name {
                        //Make sure the start stop pos difference is less than 100m
                        segment_ref.start_stop_equal(&s)
                    } else {
                        false
                    }
                }) {
                    let elapsed_sec = segment.elapsed_time / 1000;
                    let formatted_option = format!(
                        "{} -- {} -- {}",
                        segment.name,
                        format!(
                            "{:02}:{:02}:{:02}",
                            elapsed_sec / 3600,
                            (elapsed_sec / 60) % 60,
                            elapsed_sec % 60
                        ),
                        DateTime::<Utc>::from_timestamp(segment.start_time.into(), 0)
                            .unwrap()
                            .format("%m/%d/%Y")
                            .to_string()
                    );
                    fpn_vs_time.push(SegChoice {
                        file_name: entry.file_name().into_string().unwrap().replace(".bin", ""),
                        seg_time: segment.elapsed_time,
                        date_ran: segment.start_time,
                        label: formatted_option,
                    });
                }
            }
        }
    }
    return Ok(fpn_vs_time);
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

fn dist_btwn_points<F>(point1: (F, F), point2: (F, F)) -> F
where
    F: RealField + Copy,
{
    //get into n_algebra structs
    let point_1 = Point2::new(point1.0, point1.1);
    let point_2 = Point2::new(point2.0, point2.1);
    let point_vec: Vector2<F> = point_2 - point_1;

    let dot = point_vec.dot(&point_vec);

    dot.sqrt()
}
