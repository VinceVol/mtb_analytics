use std::{io, path::Path};

use inquire::Select;

use crate::{
    activity::Activity,
    data_comp::{
        split_gap::{GapTrack, GapVec},
        visuals::{generate_track_geojson, open_map_in_browser},
    },
    segments::{Segment, avail_seg_act, list_segments},
};

mod activity;
mod data_comp;
mod segments;

pub static FIT_LOC: &'static str = "./Data/";
pub static BIN_SAVE_LOC: &'static str = "./BIN_DATA/";
pub static VIDEO_LOC: &'static str = "./Video/";
pub static SEGMENT_LOC: &'static str = "./SEGMENTS/";

fn main() {
    //Make sure the proper folders exists
    std::fs::create_dir_all(FIT_LOC).expect("unable to create Data Dir in release build");
    std::fs::create_dir_all(VIDEO_LOC).expect("unable to create Data Dir in release build");
    std::fs::create_dir_all(BIN_SAVE_LOC).expect("BINCODE folder unable to be created");
    std::fs::create_dir_all(SEGMENT_LOC).expect("SEGMENT folder unable to be created");

    //Refresh the data folder for any new uploads
    Activity::refresh_bin().expect("Unable to refresh bin");

    //user selects this based on those available -- this opens up other options
    let mut segment_to_compare: Option<String> = None;
    loop {
        match segment_to_compare {
            Some(_) => {
                println!(
                    "Choose a route \n 'SL' = List available Seg \n 'cho' PR vs chosen \n 'q' = quit"
                );
            }
            None => {
                println!("Choose a route \n 'SL' = List available Seg \n 'q' = quit");
            }
        }
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        match input.trim().to_lowercase().as_str() {
            "sl" => {
                let options = list_segments().expect("Unable to list segments");
                println!("-----------------------------------------------------");
                let ans = Select::new("Select a segment from list:", options).prompt();

                match ans {
                    Ok(choice) => segment_to_compare = Some(choice),
                    Err(_) => println!("Error or cancelled (Esc/Ctrl+C)."),
                }
                println!("-----------------------------------------------------");
            }
            "cho" => {
                if segment_to_compare.is_none() {
                    println!("Still need to choose a segment!");
                    continue;
                }
                //Pull the gate data for the chosen segment
                let seg_ref = Segment::check_seg(&segment_to_compare.as_ref().unwrap())
                    .expect("issues opening segment");

                //Generate list of runs that did the segment without pausing that come within
                // 0.5km of the ref segment
                let mut run_list = avail_seg_act(&segment_to_compare.as_ref().unwrap()).unwrap();

                //For the PR run grab the shortest time
                run_list.sort_by(|item1, item2| item1.seg_time.cmp(&item2.seg_time));
                let pr_run = run_list.first().unwrap();

                println!("-----------------------------------------------------");
                //pull the pr_activity
                let pr_activity = Activity::open_bin(&pr_run.file_name).unwrap();
                println!("PR SEG: {}", pr_run.label);

                //for the chosen run sort by latest date ran
                run_list.sort_by(|item1, item2| item2.date_ran.cmp(&item1.date_ran));
                let ans = Select::new("Select which activity to compare to PR:", run_list)
                    .prompt()
                    .unwrap();
                let chosen_activity = Activity::open_bin(&ans.file_name).unwrap();
                println!("-----------------------------------------------------");

                //Run actual comparison
                //Starting with just one gap size for now
                let gates = seg_ref.large_gap;
                let pr_gate_vec = GapVec::new(
                    &gates,
                    &pr_activity
                        .segmented_activity(&segment_to_compare.as_ref().unwrap())
                        .unwrap(),
                );
                let chosen_gate_vec = GapVec::new(
                    &gates,
                    &chosen_activity
                        .segmented_activity(&segment_to_compare.as_ref().unwrap())
                        .unwrap(),
                );
                let gap_track = GapTrack::compare_gaps(chosen_gate_vec, pr_gate_vec).unwrap();
                // dbg!(&gap_track.labels);

                // Pass labels into geojson generator
                let geojson = generate_track_geojson(
                    &gap_track.data,
                    "Split Gap (s)",
                    Some(&gap_track.labels),
                    Some((-30.0, 30.0)),
                );

                let output_file = Path::new("test_map.html");
                println!("Writing HTML map to: {:?}", output_file);

                let result = open_map_in_browser(&geojson, "Split Gap (s)", output_file);
                assert!(result.is_ok(), "Failed to create or open map file");
                assert!(output_file.exists(), "HTML map file was not saved to disk");
            }
            "q" => {
                break;
            }
            _ => println!("Improper input!"),
        }
    }
}
