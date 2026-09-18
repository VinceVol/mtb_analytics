use std::{io, path::Path, sync::mpsc::Receiver};

use inquire::Select;

use crate::{
    activity::Activity,
    data_comp::{
        split_gap::{GapTrack, GapVec},
        visuals::{generate_track_geojson, open_map_in_browser},
    },
    listener::{WebMessage, start_http_listener},
    segments::{Segment, avail_seg_act, list_segments},
    video::VideoFolder,
};

use ffmpeg_sidecar::download::auto_download;

mod activity;
mod data_comp;
mod gate;
mod listener;
mod segments;
mod slope;
mod video;

pub static FIT_LOC: &'static str = "./Data/";
pub static BIN_SAVE_LOC: &'static str = "./BIN_DATA/";
pub static SEGMENT_LOC: &'static str = "./SEGMENTS/";

fn main() {
    //Make sure the proper folders exists
    std::fs::create_dir_all(FIT_LOC).expect("unable to create Data Dir in release build");
    std::fs::create_dir_all(BIN_SAVE_LOC).expect("BINCODE folder unable to be created");
    std::fs::create_dir_all(SEGMENT_LOC).expect("SEGMENT folder unable to be created");

    //init ffmpeg -- video editing tool ported into rust
    auto_download().unwrap();

    //refresh the available videos
    VideoFolder::open().unwrap();

    //Refresh the data folder for any new uploads
    Activity::refresh_bin().expect("Unable to refresh bin");

    //Start the html listener
    let rx = start_http_listener(8000);

    //user selects this based on those available -- this opens up other options
    let mut segment_to_compare: Option<String> = None;
    let mut usr_options = vec!["Q = Quit", "LS = List Available Segments"];
    loop {
        if let Ok(input) = Select::new("Choose a route", usr_options.clone()).prompt() {
            match input {
                "LS = List Available Segments" => {
                    let options = list_segments().expect("Unable to list segments");
                    println!("-----------------------------------------------------");
                    let ans = Select::new("Select a segment from list:", options).prompt();

                    match ans {
                        Ok(choice) => {
                            segment_to_compare = Some(choice);
                            if !usr_options
                                .contains(&&"PR = Compare chosen to PR".to_string().as_str())
                            {
                                usr_options.push("PR = Compare chosen to PR");
                            }
                            if !usr_options
                                .contains(&&"2U = Compare chosen to chosen".to_string().as_str())
                            {
                                usr_options.push("2U = Compare chosen to chosen");
                            }
                        }
                        Err(_) => println!("Error or cancelled (Esc/Ctrl+C)."),
                    }
                    println!("-----------------------------------------------------");
                }
                "PR = Compare chosen to PR" => {
                    //Pull the gate data for the chosen segment
                    // let seg_ref = Segment::check_seg(&segment_to_compare.as_ref().unwrap())
                    //     .expect("issues opening segment");

                    //Generate list of runs that did the segment without pausing that come within
                    // 0.5km of the ref segment
                    let mut run_list =
                        avail_seg_act(&segment_to_compare.as_ref().unwrap()).unwrap();

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
                    activate_plots(
                        pr_activity,
                        chosen_activity,
                        segment_to_compare.clone(),
                        &rx,
                    );
                }
                "2U = Compare chosen to chosen" => {
                    if segment_to_compare.is_none() {
                        println!("Still need to choose a segment!");
                        continue;
                    }
                    //Pull the gate data for the chosen segment
                    let seg_ref = Segment::check_seg(&segment_to_compare.as_ref().unwrap())
                        .expect("issues opening segment");

                    let mut run_list =
                        avail_seg_act(&segment_to_compare.as_ref().unwrap()).unwrap();
                    println!("-----------------------------------------------------");
                    //for the chosen run sort by latest date ran
                    run_list.sort_by(|item1, item2| item2.date_ran.cmp(&item1.date_ran));
                    let ans =
                        Select::new("Select the first activity to compare:", run_list.clone())
                            .prompt()
                            .unwrap();
                    let chosen_activity_1 = Activity::open_bin(&ans.file_name).unwrap();
                    println!("-----------------------------------------------------");
                    //for the chosen run sort by latest date ran
                    let ans = Select::new("Select the second activity to compare:", run_list)
                        .prompt()
                        .unwrap();
                    let chosen_activity_2 = Activity::open_bin(&ans.file_name).unwrap();
                    println!("-----------------------------------------------------");

                    activate_plots(
                        chosen_activity_1,
                        chosen_activity_2,
                        segment_to_compare.clone(),
                        &rx,
                    );
                }
                "Q = Quit" => {
                    break;
                }
                _ => println!("Improper input!"),
            }
        }
    }
}

fn activate_plots(
    A1: Activity,
    A2: Activity,
    segment_to_compare: Option<String>,
    rx: &Receiver<WebMessage>,
) {
    //Run actual comparison
    let seg_ref =
        Segment::check_seg(&segment_to_compare.as_ref().unwrap()).expect("issues opening segment");
    let gates = [seg_ref.small_gap, seg_ref.med_gap, seg_ref.large_gap];
    let mut results = vec![];

    let mut first_gate_vecs = vec![];
    let mut second_gate_vecs = vec![];

    for (graph_ind, gate) in gates.iter().enumerate() {
        let first_gate_vec = GapVec::new(
            &gate,
            &A1.segmented_activity(&segment_to_compare.as_ref().unwrap())
                .unwrap(),
        );
        let second_gate_vec = GapVec::new(
            &gate,
            &A2.segmented_activity(&segment_to_compare.as_ref().unwrap())
                .unwrap(),
        );
        let gap_track =
            GapTrack::compare_gaps(first_gate_vec.clone(), second_gate_vec.clone()).unwrap();
        first_gate_vecs.push(first_gate_vec);
        second_gate_vecs.push(second_gate_vec);
        // dbg!(&gap_track.labels);

        // Pass labels into geojson generator
        let geojson = generate_track_geojson(
            &gap_track.data,
            "Split Gap (s)",
            None, //Some(&gap_track.labels),
            Some(&gate),
            // Some((-0.0, 10.0)),
        );

        match graph_ind {
            0 => {
                results.push((geojson, "Split Gap (s)", "Small Gap"));
            }
            1 => {
                results.push((geojson, "Split Gap (s)", "Medium Gap"));
            }
            2 => {
                results.push((geojson, "Split Gap (s)", "Large Gap"));
            }
            _ => (),
        }
    }
    let _ = open_map_in_browser(&results, Path::new("results.html"), "Chosen vs PR", 8000);
    let v_f = VideoFolder::open().unwrap();

    println!("Listening for browser events...");
    for msg in rx {
        match msg {
            listener::WebMessage::MapClick { lat, lng } => {
                println!("Clicked map at coordinates: {}, {}", lat, lng);
            }
            listener::WebMessage::GateClicked {
                gate_id,
                dataset_name,
            } => {
                println!("Gate clicked: {}", gate_id);
                println!("sheet: {}", dataset_name);
                match dataset_name.as_str() {
                    "Small Gap" => {
                        let _ = v_f.compare(&first_gate_vecs[0], &second_gate_vecs[0], gate_id);
                    }
                    "Medium Gap" => {
                        let _ = v_f.compare(&first_gate_vecs[1], &second_gate_vecs[1], gate_id);
                    }
                    "Large Gap" => {
                        let _ = v_f.compare(&first_gate_vecs[2], &second_gate_vecs[2], gate_id);
                    }
                    _ => println!("Invalid input for vid gen"),
                }
            }
            listener::WebMessage::SegmentSelected(seg) => {
                println!("Segment selected: {}", seg);
            }
            listener::WebMessage::Unknown { action, payload } => {
                println!("Received custom event '{}': {:?}", action, payload);
            }
        }
    }
}
