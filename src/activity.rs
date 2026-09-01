use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use fit::{Fit, Value};

use rkyv::{Archive, Deserialize, Serialize, access, deserialize, rancor};

use crate::{BIN_SAVE_LOC, FIT_LOC};

// Dense time-series: stored in contiguous memory (Structure of Arrays)
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct TrackpointDataFrame {
    pub timestamps: Vec<Option<u32>>, // Unix timestamps (seconds)
    pub distance_m: Vec<Option<u32>>, // Distance in meters
    pub speed_kmh: Vec<Option<f32>>,  // Converted speed
    pub heart_rate: Vec<Option<u8>>,  // Direct bpm
    pub latitude: Vec<Option<f32>>,   // Decimal degrees
    pub longitude: Vec<Option<f32>>,  // Decimal degrees
    pub altitude_m: Vec<Option<u32>>, // Meters
    pub slope_pct: Vec<Option<f32>>,  // Derived slope %
}

// Sparse event metadata
#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct SegmentRef {
    pub name: Option<String>,
    pub start_time: Option<u32>,
    pub elapsed_time: Option<u32>, //milliseconds
    pub distance: Option<u32>,     // 1over100m
}

#[derive(Debug, Archive, Serialize, Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug))]
pub struct Activity {
    pub metadata_id: String,
    pub segments: Vec<SegmentRef>,
    pub telemetry: TrackpointDataFrame,
}

impl Activity {
    pub fn refresh_bin() -> Result<(), Box<dyn std::error::Error>> {
        //Start by figuring out what bins already exist
        let mut cur_bins: Vec<String> = Vec::new();
        for entry_res in fs::read_dir(BIN_SAVE_LOC)? {
            if let Ok(entry) = entry_res {
                cur_bins.push(
                    entry
                        .file_name()
                        .to_str()
                        .unwrap()
                        .to_owned()
                        .replace(".bin", ""),
                );
            }
        }

        //iterate through fit files and add the ones that don't exist
        for entry_res in fs::read_dir(FIT_LOC)? {
            if let Ok(entry) = entry_res {
                if !cur_bins.contains(&entry.file_name().into_string().unwrap().replace(".fit", ""))
                {
                    let activity = Activity::add(entry.path());
                    activity.save_bin()?;
                }
            }
        }
        Ok(())
    }
    fn add(fp: PathBuf) -> Self {
        //Trackpoint data frame initialization
        let mut trackpoint_dataframe = TrackpointDataFrame {
            timestamps: Vec::new(),
            distance_m: Vec::new(),
            speed_kmh: Vec::new(),
            heart_rate: Vec::new(),
            latitude: Vec::new(),
            longitude: Vec::new(),
            altitude_m: Vec::new(),
            slope_pct: Vec::new(),
        };
        //segment data
        let mut segments: Vec<SegmentRef> = Vec::new();
        let fit = Fit::new(&fp);
        for m in fit {
            match m.kind {
                //Save segment info
                fitsdk::MessageType::SegmentLap => {
                    let mut name: Option<String> = None; //29
                    let mut start_time: Option<u32> = None; //2 
                    let mut elapsed_time: Option<u32> = None; // 7
                    let mut distance: Option<u32> = None; //9
                    for field in m.values {
                        match field.field_num {
                            29 => {
                                if let Value::String(name_str) = field.value {
                                    name = Some(name_str);
                                }
                            }
                            2 => {
                                if let Value::Time(date_time) = field.value {
                                    start_time = Some(date_time);
                                }
                            }
                            7 => {
                                if let Value::U32(date_time) = field.value {
                                    elapsed_time = Some(date_time);
                                }
                            }
                            9 => {
                                if let Value::U32(dist) = field.value {
                                    distance = Some(dist);
                                }
                            }
                            _ => {}
                        }
                    }
                    if name.is_some() && start_time.is_some() && elapsed_time.is_some() {
                        segments.push(SegmentRef {
                            name,
                            start_time,
                            elapsed_time,
                            distance,
                        })
                    }
                }
                //Save time info
                fitsdk::MessageType::Record => {
                    // dbg!(&m.values);
                    let mut timestamp: Option<u32> = None; //253
                    let mut distance_m: Option<u32> = None; //5
                    let mut speed_kmh: Option<f32> = None; //6
                    let mut heart_rate: Option<u8> = None; //3  
                    let mut latitude: Option<f32> = None; //0   
                    let mut longitude: Option<f32> = None; //1 
                    let mut altitude_m: Option<u32> = None; //78
                    let mut slope_pct: Option<f32> = None;

                    for field in m.values {
                        match field.field_num {
                            253 => {
                                if let Value::Time(timestmp) = field.value {
                                    timestamp = Some(timestmp);
                                }
                            }
                            5 => {
                                if let Value::U32(distance) = field.value {
                                    distance_m = Some(distance);
                                }
                            }
                            73 => {
                                if let Value::U32(speed) = field.value {
                                    speed_kmh = Some((speed as f32) / 1000.0);
                                }
                            }
                            3 => {
                                if let Value::U8(hr) = field.value {
                                    heart_rate = Some(hr);
                                }
                            }

                            0 => {
                                if let Value::F32(lat) = field.value {
                                    latitude = Some(lat);
                                }
                            }
                            1 => {
                                if let Value::F32(long) = field.value {
                                    longitude = Some(long);
                                }
                            }
                            78 => {
                                if let Value::U32(alt) = field.value {
                                    altitude_m = Some(alt);
                                }
                            }
                            _ => {}
                        }
                    }
                    trackpoint_dataframe.timestamps.push(timestamp);
                    trackpoint_dataframe.distance_m.push(distance_m);
                    trackpoint_dataframe.speed_kmh.push(speed_kmh);
                    trackpoint_dataframe.heart_rate.push(heart_rate);
                    trackpoint_dataframe.latitude.push(latitude);
                    trackpoint_dataframe.longitude.push(longitude);
                    trackpoint_dataframe.altitude_m.push(altitude_m);
                    trackpoint_dataframe.slope_pct.push(slope_pct);
                }
                _ => continue,
            }
        }
        Self {
            metadata_id: fp
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
                .replace(".fit", ""),
            segments: segments,
            telemetry: trackpoint_dataframe,
        }
    }

    pub fn save_bin(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<rancor::Error>(self)?;

        let fp = format!("{}{}.bin", BIN_SAVE_LOC, self.metadata_id);
        let mut the_file = std::fs::File::create(fp)?;
        the_file.write_all(&bytes)?;
        Ok(())
    }
    pub fn open_bin(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let fp = format!("{}{}.bin", BIN_SAVE_LOC, name);
        let mut file = std::fs::File::open(fp)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        // let archived = access::<ArchivedActivity, rancor::Error>(&bytes)?;
        let archived = rkyv::access::<ArchivedActivity, rancor::Error>(&bytes[..]).unwrap();
        let activity: Activity = deserialize::<Activity, rancor::Error>(archived)?;
        Ok(activity)
    }
}

#[cfg(test)]
mod d_test {
    use std::path::PathBuf;

    use super::*;
    #[test]
    fn test_fit() {
        // dbg!(Activity::add(PathBuf::from(
        //     "./Data/thatonematrixman@gmail.com_470194221701.fit",
        // )));
        Activity::refresh_bin();
        assert_eq!(false, true);
    }
}
