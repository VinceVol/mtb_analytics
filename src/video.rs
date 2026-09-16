//Basically we dont want to reinvent the wheel here. We want to port functions from FFmpeg by legit
// formatting and executing terminal commands

use std::path::PathBuf;

struct Video {
    fp: PathBuf,
}

impl Video {
    fn find_video(target_time: u32) -> Video {
        todo!();
    }
}
