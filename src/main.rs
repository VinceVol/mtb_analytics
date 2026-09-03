use std::io;

use inquire::Select;

use crate::{
    activity::Activity,
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
                //Generate list of runs that did the segment without pausing that come within
                // 0.5km of the ref segment
                let mut run_list = avail_seg_act(&segment_to_compare.as_ref().unwrap()).unwrap();

                //For the PR run grab the shortest time
                run_list.sort_by(|item1, item2| item1.seg_time.cmp(&item2.seg_time));
                let pr_run = run_list.first().unwrap();

                println!("-----------------------------------------------------");
                //pull the pr_activity
                let pr_activity = Activity::open_bin(&pr_run.file_name);
                println!("PR SEG: {}", pr_run.label);

                //for the chosen run sort by latest date ran
                run_list.sort_by(|item1, item2| item2.date_ran.cmp(&item1.date_ran));
                let ans = Select::new("Select which activity to compare to PR:", run_list)
                    .prompt()
                    .unwrap();
                let chosen_activity = Activity::open_bin(&ans.file_name);
                println!("-----------------------------------------------------");
            }
            "q" => {
                break;
            }
            _ => println!("Improper input!"),
        }
    }
}
