use std::io;

use inquire::Select;

use crate::segments::list_segments;

mod activity;
mod data_comp;
mod segments;

pub static FIT_LOC: &'static str = "./Data/";
pub static BIN_SAVE_LOC: &'static str = "./BIN_DATA/";
pub static VIDEO_LOC: &'static str = "./Video/";

fn main() {
    //Make sure the proper folders exists
    std::fs::create_dir_all(FIT_LOC).expect("unable to create Data Dir in release build");
    std::fs::create_dir_all(VIDEO_LOC).expect("unable to create Data Dir in release build");
    std::fs::create_dir_all(BIN_SAVE_LOC).expect("BINCODE folder unable to be created");

    //user selects this based on those available -- this opens up other options
    let mut segment_to_compare: Option<String> = None;
    loop {
        match segment_to_compare {
            Some(_) => {
                println!(
                    "Choose a route \n 'SL' = List available Seg \n 'Lat' PR vs Latest \n 'q' = quit"
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
                println!("-----------------------------------------------------");
                let options = list_segments().expect("Unable to list segments");
                println!("-----------------------------------------------------");
                let ans = Select::new("Select a segment from list:", options).prompt();

                match ans {
                    Ok(choice) => segment_to_compare = Some(choice),
                    Err(_) => println!("Error or cancelled (Esc/Ctrl+C)."),
                }
                println!("-----------------------------------------------------");
                // new_descriptor_file(&cur_files);
            }
            "lat" => {
                if segment_to_compare.is_none() {
                    println!("Still need to choose a segment!");
                    continue;
                }
                todo!();
            }
            "q" => {
                break;
            }
            _ => println!("Improper input!"),
        }
    }
}
