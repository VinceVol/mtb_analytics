//Basically we dont want to reinvent the wheel here. We want to port functions from FFmpeg by legit
// formatting and executing terminal commands

use std::{
    fs::{self, DirEntry},
    io::{Read, Write},
    time::UNIX_EPOCH,
};

use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};

//Setting up the video folder such that it's saved in a binary and if the folder isn't found the user
// can be prompted to find a new folder
#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct VideoFolder {
    fp: Option<String>,
    videos: Vec<Video>,
}

#[derive(Debug, Archive, Serialize, Deserialize, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct Video {
    fp: String, //Need String instead of pathbuf for rkyv Archive/Serialize/Deserialize
    start_time: u32,
    activity_ref: String, //cant do &activity in a rkyv
}

impl Video {
    fn new(file: DirEntry) -> Result<Self, Box<dyn std::error::Error>> {
        let fp = file.path().to_str().unwrap().to_owned();
        let start_time: u32 = file
            .metadata()?
            .created()?
            .duration_since(UNIX_EPOCH)?
            .as_secs()
            .try_into()?;

        todo!()
    }
}

static VIDEO_LOC: &'static str = "./video_fldr.bin";
impl VideoFolder {
    fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(VIDEO_LOC)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        //Check if there's a saved Video Folder binary if not make one
        let mut archived = match rkyv::access::<ArchivedVideoFolder, rancor::Error>(&bytes[..]) {
            Ok(archived_ok) => deserialize::<VideoFolder, rancor::Error>(archived_ok)?,
            Err(_e) => {
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
                                let vid = Video::new(entry);
                                archived.videos.push(vid);
                            }
                        }
                    }
                    //now that the files have been looked through again break out
                    return Ok(archived);
                }
                None => {
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
}
