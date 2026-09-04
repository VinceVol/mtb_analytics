//Common +/- Green red

use crate::{activity::Activity, segments::Gate};

pub struct GapTrack {
    data: Vec<(f32, f32, f32)>,
    labels: Vec<(f32, f32, String)>,
}

pub struct GapVec {
    gap_vec: Vec<Option<u32>>,
    gate_gps_index: Vec<Option<Vec<(f32, f32)>>>, //Need to store the telemetry index when crossing the gate
}

impl GapVec {
    pub fn new(gates: Vec<Gate>, activity_ref: &Activity) -> Self {
        let mut split_times: Vec<Option<u32>> = Vec::new();
        let mut gate_gps_index: Vec<Option<Vec<(f32, f32)>>> = Vec::new();
        let mut pp_long = None; //previous longitude
        let mut pp_lat = None; //previous latitude
        let mut last_suc_ind: usize = 0; //track the index you left off at
        let mut gps_data: Vec<(f32, f32)> = Vec::new(); //dump telemetry data in here
        let mut success = false; //Track whether intersection was found
        for gate in gates {
            //track whether data point was saved
            success = false;
            for (index, long) in activity_ref.telemetry.longitude.iter().enumerate() {
                //add data to gps coord vec
                let lat = activity_ref.telemetry.latitude[index];
                if lat.is_some() && long.is_some() {
                    gps_data.push((long.unwrap(), lat.unwrap()));
                }

                //dont go back through the front end of activity telemetry every time you
                // cycle through to find the intersection of a gate
                if index == 0 || index <= last_suc_ind {
                    pp_long = *long;
                    pp_lat = lat;
                    continue;
                }
                if pp_long.is_some() && pp_lat.is_some() && long.is_some() && lat.is_some() {
                    //construct the line we want to check going through the gate
                    let current_point = (long.unwrap(), lat.unwrap());
                    let prev_point = (pp_long.unwrap(), pp_lat.unwrap());
                    let points = [prev_point, current_point];

                    //check if the gate was crossed by that new line (points)
                    if gate.is_crossed(points) {
                        split_times.push(activity_ref.telemetry.timestamps[index]); //add the time so we can compare later
                        gate_gps_index.push(Some(gps_data.clone())); //needed for visuals
                        gps_data.clear();
                        last_suc_ind = index;
                        success = true;
                        break;
                    }
                }
            }
            if !success {
                split_times.push(None);
                gate_gps_index.push(None);
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
    fn compare_gaps(
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
                differences.push(Some(gap_time_2.unwrap() - gap_time_1.unwrap()));
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
                        labels.push((*long, *lat, differences[index].unwrap_or(0).to_string()));
                    }
                    //show a difference of 0 if that difference didnt exist
                    // kind of a flaw for now but whatever. Eventually I should
                    // print missing data purple or something TODO
                    data.push((*long, *lat, differences[index].unwrap_or(0) as f32));
                }
            }
        }
        Ok(GapTrack { data, labels })
    }
}
