use chrono::{DateTime, Utc};
use std::{
    fmt, fs,
    io::{Read, Write},
};

use crate::{
    BIN_SAVE_LOC, SEGMENT_LOC,
    activity::{Activity, SegmentRef},
    gate::{Gate, dist_btwn_points, points_to_utm},
};
use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};

//Starting with gaps -- basically split gaps at different intervals as I think categorizing
// turns may be a difficult starting point not worth digging into right at the start
// I do want to add uphills and downhills later
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Segment {
    name: String,
    ref_length: u32,          //meters/100
    pub small_gap: Vec<Gate>, //every 5 readings on ref
    pub med_gap: Vec<Gate>, //every 20 readings on ref usize(as u64 cuz of archive) points to a gate index in small gap
    pub large_gap: Vec<Gate>, //every 60 readings on ref
    start_end_pos: [(f32, f32); 2], //reference to determine whether the segment was finished
                            // uphills: Vec<(gate,gate)>,
                            // downhills: Vec<(gate,gate)>,
}

impl Segment {
    fn new(ref_activity: &Activity, seg_name: &str) -> Result<Segment, Box<dyn std::error::Error>> {
        //start with some blank vectors to fill

        let mut small_gap: Vec<Gate> = Vec::new(); //every 5 readings on ref
        let mut med_gap: Vec<Gate> = Vec::new(); //every 20 readings on ref
        let mut large_gap: Vec<Gate> = Vec::new(); //every 60 readings on ref

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
                    let gate = Gate::new(three_points, 15.0);
                    if (i as f32 / 20.0) == (i as f32 / 20.0) as usize as f32 {
                        med_gap.push(gate.clone());
                    }
                    if (i as f32 / 60.0) == (i as f32 / 60.0) as usize as f32 {
                        large_gap.push(gate.clone());
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
                    //Doing all the formatting for displaying an activity segment here but should make this available for
                    // printing out activity data directly from the Activity struct itself TODO
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
