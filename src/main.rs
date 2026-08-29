use std::io;

mod activity;
mod segments;

fn main() {
    //Make sure the proper folders exists
    std::fs::create_dir_all("./Data/").expect("unable to create Data Dir in release build");
    std::fs::create_dir_all("./Video/").expect("unable to create Data Dir in release build");
    loop {
        println!(
            "Choose a route \n 'SL' = List available Seg \n 'S: <>' = Choose Seg vs Latest \n 'C' = Compare Folder Files \n 'q' = quit"
        );
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        match input.trim().to_lowercase().as_str() {
            "SL" => {
                todo!();
                // new_descriptor_file(&cur_files);
            }
            "C" => {
                todo!();
            }
            "q" => {
                break;
            }
            alternate_input => {
                //brief input filtering -- only need partial answer from user
                if !alternate_input.contains("S:") {
                    println!("IMPROPER INPUT");
                    continue;
                }
                let mut seg_in = alternate_input.replace("S:", "");
                seg_in = seg_in.replace(" ", "");
                todo!();
            }
        }
    }
}
