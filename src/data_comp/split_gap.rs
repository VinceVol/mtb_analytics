//Common +/- Green red

use crate::{activity::Activity, gate::Gate};

#[derive(Debug)]
pub struct GapTrack {
    pub data: Vec<(f32, f32, f32)>,
    pub labels: Vec<(f32, f32, String)>,
}

#[derive(Debug)]
pub struct GapVec {
    pub gap_vec: Vec<Option<u32>>,
    gate_gps_index: Vec<Option<Vec<(f32, f32)>>>, //Need to store the telemetry index when crossing the gate
}

impl GapVec {
    pub fn new(gates: &Vec<Gate>, activity_ref: &Activity) -> Self {
        let mut split_times: Vec<Option<u32>> = Vec::new();
        let mut gate_gps_index: Vec<Option<Vec<(f32, f32)>>> = Vec::new();
        let mut pp_long = None; //previous longitude
        let mut pp_lat = None; //previous latitude
        let mut last_suc_ind: usize = 0; //track the index you left off at
        let mut gps_data: Vec<(f32, f32)> = Vec::new(); //dump telemetry data in here
        let mut success; //Track whether intersection was found
        for (gate_ind, gate) in gates.iter().enumerate() {
            //track whether data point was saved
            success = false;

            //As a backup for missed gates append the distance from gate to choose the point w/
            // min distance if the gate was never crossed
            let mut min_dist = Vec::new();
            for (index, long) in activity_ref.telemetry.longitude.iter().enumerate() {
                let lat = activity_ref.telemetry.latitude[index];
                //dont go back through the front end of activity telemetry every time you
                // cycle through to find the intersection of a gate
                if index == 0 || index <= last_suc_ind {
                    pp_long = *long;
                    pp_lat = lat;
                    continue;
                }

                //add data to gps coord vec
                if lat.is_some() && long.is_some() {
                    gps_data.push((long.unwrap(), lat.unwrap()));
                }

                if pp_long.is_some() && pp_lat.is_some() && long.is_some() && lat.is_some() {
                    //construct the line we want to check going through the gate
                    let current_point = (long.unwrap(), lat.unwrap());
                    let prev_point = (pp_long.unwrap(), pp_lat.unwrap());
                    let points = [prev_point, current_point];

                    //check if the gate was crossed by that new line (points)
                    if gate.is_crossed(points) {
                        split_times.push(Some(
                            activity_ref.telemetry.timestamps[index].unwrap()
                                - activity_ref.telemetry.timestamps[last_suc_ind].unwrap(),
                        ));
                        gate_gps_index.push(Some(gps_data.clone())); //needed for visuals
                        gps_data.clear();
                        last_suc_ind = index;
                        success = true;
                        break;
                    } else {
                        min_dist.push((
                            gate.dist_to_center(long.unwrap(), lat.unwrap()),
                            index,
                            gps_data.len(),
                        ));
                    }
                }
            }
            if !success {
                //gps_data index is there to tell us what to save up to since we appended
                // a bunch of points after we passed the min distance in the last loop
                min_dist.sort_by(|(d1, _, _), (d2, _, _)| d1.total_cmp(d2));
                if let Some((min_dist, min_index, gps_data_ind)) = min_dist.first()
                    && *min_dist < 10.0
                {
                    split_times.push(Some(
                        activity_ref.telemetry.timestamps[*min_index].unwrap()
                            - activity_ref.telemetry.timestamps[last_suc_ind].unwrap(),
                    ));
                    gate_gps_index.push(Some(gps_data[0..*gps_data_ind].to_vec())); //needed for visuals
                    gps_data.clear();
                    last_suc_ind = *min_index;
                } else {
                    println!("Gate #{} was never crossed", gate_ind);
                    split_times.push(None);
                    gate_gps_index.push(None);
                }
            }
        }
        assert_eq!(split_times.len(), gate_gps_index.len());
        return Self {
            gap_vec: split_times,
            gate_gps_index,
        };
    }
}

impl GapTrack {
    //comparing gap 2 - gap 1
    //returning gap 2 track
    pub fn compare_gaps(
        gap_vec_1: GapVec,
        mut gap_vec_2: GapVec,
    ) -> Result<GapTrack, Box<dyn std::error::Error>> {
        //Make sure the comparison of two things of similar length so you dont break everything
        if gap_vec_1.gap_vec.len() != gap_vec_2.gap_vec.len() {
            return Err("The two gaps being compared are different lengths".into());
        }

        let mut differences = Vec::new();
        for (index, gap_time_1) in gap_vec_1.gap_vec.iter().enumerate() {
            let gap_time_2 = gap_vec_2.gap_vec[index];

            //compare the difference if they both have a number to show for it
            if gap_time_1.is_some() && gap_time_2.is_some() {
                differences.push(Some(
                    gap_time_2.unwrap() as i32 - gap_time_1.unwrap() as i32,
                ));
            } else {
                differences.push(None);
            }
        }

        //After gathering those differences go back through gap 2 and generate the labels
        // and GapTrack for visuals
        let mut data = Vec::new();
        let mut labels = Vec::new();

        for (index, tele_data) in gap_vec_2.gate_gps_index.iter_mut().enumerate() {
            if tele_data.is_some() {
                for (tele_index, (long, lat)) in tele_data.as_mut().unwrap().iter().enumerate() {
                    if tele_index == 0 {
                        labels.push((*lat, *long, differences[index].unwrap_or(0).to_string()));
                    }
                    //show a difference of 0 if that difference didnt exist
                    // kind of a flaw for now but whatever. Eventually I should
                    // print missing data purple or something TODO
                    data.push((*lat, *long, differences[index].unwrap_or(0) as f32));
                }
            }
        }
        Ok(GapTrack { data, labels })
    }
}
