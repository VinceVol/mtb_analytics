//Basically we dont want to reinvent the wheel here. We want to port functions from FFmpeg by legit
// formatting and executing terminal commands

use chrono::Utc;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};
use serde_json::Value;
use std::process::Command;
use std::{
    fmt::Debug,
    fs::{self, DirEntry},
    io::{Read, Write},
    time::UNIX_EPOCH,
};

use crate::data_comp::split_gap::GapVec;
use crate::gate::Gate;
use crate::{BIN_SAVE_LOC, activity::Activity};

//Setting up the video folder such that it's saved in a binary and if the folder isn't found the user
// can be prompted to find a new folder
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct VideoFolder {
    fp: Option<String>,
    videos: Vec<Video>,
}

#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Video {
    fp: String, //Need String instead of pathbuf for rkyv Archive/Serialize/Deserialize
    start_time: u32,
    end_time: u32,
    activity_ref: Option<String>, //cant do &activity in a rkyv
}

impl Video {
    fn new(file: DirEntry) -> Result<Self, Box<dyn std::error::Error>> {
        let fp = file.path().to_str().unwrap().to_owned();

        //We need ffprobe to tell us when the video was filmed as DirEntry doesn't contain that metadata
        let output = Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format", &fp])
            .output()?;

        let json: Value = serde_json::from_slice(&output.stdout)?;
        let creation_time = json["format"]["tags"]["creation_time"]
            .as_str()
            .ok_or(format!("Could not find video |{}| creation time", &fp))?;
        let duration = json["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or(format!("Could not find video |{}| duration", &fp))?
            as u32;

        let dt = chrono::DateTime::parse_from_rfc3339(creation_time)?.with_timezone(&Utc);

        let start_time: u32 = dt.timestamp() as u32;

        let mut activity_ref: Option<String> = None;
        //Cycle through activities and see if one of them falls into the
        // right time
        for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
            if let Ok(entry) = entry_res {
                let activity_name = entry
                    .file_name()
                    .to_str()
                    .unwrap()
                    .to_owned()
                    .replace(".bin", "");
                let activity = Activity::open_bin(&activity_name)?;
                // println!(
                //     "The activity: {:?} \n had a start time of {:?}, and a end time of {:?}.\n Clip start time: {:?}",
                //     activity.metadata_id,
                //     chrono::DateTime::<Utc>::from_timestamp(
                //         activity.start_time().unwrap().into(),
                //         0
                //     )
                //     .unwrap()
                //     .format("%m/%d/%Y %H:%M")
                //     .to_string(),
                //     chrono::DateTime::<Utc>::from_timestamp(activity.end_time().unwrap().into(), 0)
                //         .unwrap()
                //         .format("%m/%d/%Y %H:%M")
                //         .to_string(),
                //     chrono::DateTime::<Utc>::from_timestamp(start_time.into(), 0)
                //         .unwrap()
                //         .format("%m/%d/%Y %H:%M")
                //         .to_string(),
                // );
                //5 min before and after the activity timing to consider it as a valid reference
                if activity.start_time()? - (60 * 5) < start_time
                    && activity.end_time()? + (60 * 5) > start_time
                {
                    activity_ref = Some(activity.metadata_id);
                    break;
                }
            }
        }

        return Ok(Self {
            fp,
            start_time,
            activity_ref,
            end_time: start_time + duration,
        });
    }
    fn contains(&self, time: u32) -> bool {
        self.start_time <= time && self.end_time >= time
    }
}

static VIDEO_LOC: &'static str = "./video_fldr.bin";
impl VideoFolder {
    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = match std::fs::File::open(VIDEO_LOC) {
            Ok(file_r) => file_r,
            Err(_) => {
                println!(
                    "Video Folder struct binary not found in the working directory! Creating the Binary now..."
                );
                let v_f = VideoFolder {
                    fp: None,
                    videos: Vec::new(),
                };
                v_f.save()?;
                std::fs::File::open(VIDEO_LOC)?
            }
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        //Check if there's a saved Video Folder binary if not make one
        let mut archived = match rkyv::access::<ArchivedVideoFolder, rancor::Error>(&bytes[..]) {
            Ok(archived_ok) => deserialize::<VideoFolder, rancor::Error>(archived_ok)?,
            Err(_e) => {
                println!(
                    "Video Folder struct binary found but corrupted! Generating a fresh Binary now..."
                );
                let v_f = VideoFolder {
                    fp: None,
                    videos: Vec::new(),
                };
                v_f.save()?;
                v_f
            }
        };

        //Open the videofolder if it is some else prompt user for new folder
        loop {
            match archived.fp.as_ref() {
                //once folder exists go through it and add any videos that
                // were not present before
                Some(folder) => {
                    for entry_res in std::fs::read_dir(folder)? {
                        if let Ok(entry) = entry_res {
                            if !archived
                                .videos
                                .iter()
                                .any(|v| v.fp.contains(entry.file_name().to_str().unwrap()))
                            {
                                match Video::new(entry) {
                                    Ok(vid) => archived.videos.push(vid),
                                    Err(e) => {
                                        println!("The video opening had this error: \n{}", e)
                                    }
                                }
                            }
                        }
                    }
                    //now that the files have been looked through again break out
                    archived.save()?; //TODO turn back on when ready
                    return Ok(archived);
                }
                None => {
                    println!("You need to select a video dumping ground folder");
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        archived.fp = Some(folder.to_str().unwrap().to_owned());
                    }
                    continue;
                }
            }
        }
    }

    fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<rancor::Error>(self)?;

        let mut the_file = std::fs::File::create(VIDEO_LOC)?;
        the_file.write_all(&bytes)?;
        Ok(())
    }

    //This should allow us to take two activities and crop up a side by side playback
    //TODO error handling in this function
    pub fn compare(
        &self,
        gv_1: &GapVec,
        gv_2: &GapVec,
        mut start_gate: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        //cant read the last gate to nothing but we can fix this by going back 1
        assert_eq!(gv_1.time_vec.len(), gv_2.time_vec.len());
        assert!(gv_1.time_vec.len() > 0);
        if start_gate == gv_1.time_vec.len() {
            start_gate -= 1;
        }

        //find the appropriate videos for the relavent gate
        let unix_start_1 = gv_1.time_vec[start_gate].unwrap_or_default();
        let unix_end_1 = gv_1.time_vec[start_gate + 1].unwrap_or_default();
        let unix_start_2 = gv_2.time_vec[start_gate].unwrap_or_default();
        let unix_end_2 = gv_2.time_vec[start_gate + 1].unwrap_or_default();
        let gv_1_video = self
            .videos
            .iter()
            .find(|v| v.contains(unix_start_1) && v.contains(unix_end_1))
            .ok_or("unable to find the first gap vec video")?;
        let gv_2_video = self
            .videos
            .iter()
            .find(|v| v.contains(unix_start_2) && v.contains(unix_end_2))
            .ok_or("unable to find the second gap vec video")?;

        //find the offsets from action cam time to real time
        let off_1: i32 = gv_1.time_vec[0].unwrap_or_default() as i32 - gv_1_video.start_time as i32;
        let off_2: i32 = gv_2.time_vec[0].unwrap_or_default() as i32 - gv_2_video.start_time as i32;
        //deduce the start and stop times of the actual mp4 files
        let vid_start_1 = (unix_start_1 as i32 - gv_1_video.start_time as i32 - off_1) as u32;
        let vid_duration_1 = unix_end_1 - unix_start_1;
        let vid_start_2 = (unix_start_2 as i32 - gv_2_video.start_time as i32 - off_2) as u32;
        let vid_duration_2 = unix_end_2 - unix_start_2;

        //format for ffmpeg
        let overlay_text = format!(
            "Gates {}-{}\n{} vs {}",
            start_gate,
            start_gate + 1,
            chrono::DateTime::<Utc>::from_timestamp(unix_start_1.into(), 0)
                .unwrap()
                .format("%m/%d/%Y")
                .to_string(),
            chrono::DateTime::<Utc>::from_timestamp(unix_start_2.into(), 0)
                .unwrap()
                .format("%m/%d/%Y")
                .to_string(),
        );
        // 2. Build filter graph
        let filter_spec = format!(
            "[0:v]trim=start={vid_start_1}:duration={vid_duration_1},setpts=PTS-STARTPTS,scale=-1:1080[v1]; \
                     [1:v]trim=start={vid_start_2}:duration={vid_duration_2},setpts=PTS-STARTPTS,scale=-1:1080[v2]; \
                     [0:a]atrim=start={vid_start_1}:duration={vid_duration_1},asetpts=PTS-STARTPTS[a1]; \
                     [1:a]atrim=start={vid_start_2}:duration={vid_duration_2},asetpts=PTS-STARTPTS[a2]; \
                     [v1][v2]hstack=inputs=2[stacked]; \
                     [stacked]drawtext=text='{overlay_text}':text_align=center:x=(w-text_w)/2:y=h-text_h-80:fontsize=36:fontcolor=white[v_out]; \
                     [a1][a2]amix=inputs=2[a_out]"
        );
        // 3. Execute FFmpeg process
        let mut child = FfmpegCommand::new()
            .input(gv_1_video.fp.clone())
            .input(gv_2_video.fp.clone())
            .filter_complex(filter_spec)
            .map("[v_out]")
            .map("[a_out]")
            .output("output.mp4")
            .spawn()?;

        // 4. Handle events/progress
        for event in child.iter()? {
            if let FfmpegEvent::Progress(progress) = event {
                println!("Processing frame: {}", progress.frame);
            }
        }

        Ok(())
    }
}
