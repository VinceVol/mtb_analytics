use std::path::PathBuf;

use fit::{Fit, Value};

use serde::{Deserialize, Serialize};

// Dense time-series: stored in contiguous memory (Structure of Arrays)
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackpointDataFrame {
    pub timestamps: Vec<i64>, // Unix timestamps (seconds)
    pub distance_m: Vec<f32>, // Distance in meters
    pub speed_kmh: Vec<f32>,  // Converted speed
    pub heart_rate: Vec<u8>,  // Direct bpm
    pub latitude: Vec<f64>,   // Decimal degrees
    pub longitude: Vec<f64>,  // Decimal degrees
    pub altitude_m: Vec<f32>, // Meters
    pub slope_pct: Vec<f32>,  // Derived slope %
}

// Sparse event metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentRef {
    pub name: String,
    pub start_dist: f32, // meters into the activity
    pub end_dist: f32,
    pub elapsed_sec: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Activity {
    pub metadata_id: String,
    pub segments: Vec<SegmentRef>,
    pub telemetry: TrackpointDataFrame,
}

impl Activity {
    fn new(fp: PathBuf) {
        let fit = Fit::new(&fp);
        for m in fit {
            match m.kind {
                //Save segment info
                fitsdk::MessageType::SegmentLap => {
                    println!("segment data!");
                    dbg!(&m.values);
                }
                //Save time info
                //field 3 is local time
                fitsdk::MessageType::Record => {
                    let mut time: Option<u32> = None; //264
                    let mut distance_m: Option<u32> = None; //5
                    let mut speed_kmh: Option<u16> = None; //6
                    let mut heart_rate: Option<u8> = None; //3  
                    let mut latitude: Option<i32> = None; //0   
                    let mut longitude: Option<i32> = None; //1 
                    let mut altitude_m: Option<u16> = None; //2
                    let mut slope_pct: Option<f32> = None;

                    for field in m.values {
                        match field.field_num {
                            264 => {
                                if let Value::Time(timestamp) = field.value {
                                    time = Some(timestamp);
                                }
                            }
                            5 => {
                                if let Value::U32(distance) = field.value {
                                    distance_m = Some(distance);
                                }
                            }
                            6 => {
                                if let Value::U16(speed) = field.value {
                                    speed_kmh = Some(speed);
                                }
                            }
                            3 => {
                                if let Value::U8(hr) = field.value {
                                    heart_rate = Some(hr);
                                }
                            }

                            0 => {
                                if let Value::I32(lat) = field.value {
                                    latitude = Some(lat);
                                }
                            }
                            1 => {
                                if let Value::I32(long) = field.value {
                                    longitude = Some(long);
                                }
                            }
                            2 => {
                                if let Value::U16(alt) = field.value {
                                    altitude_m = Some(alt);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => continue,
            }
        }
    }
    //eventually return something
}

mod d_test {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    #[test]
    fn test_fit() {
        Activity::new(PathBuf::from(
            "./Data/thatonematrixman@gmail.com_470194221701.fit",
        ));
        assert_eq!(false, true);
    }
}
