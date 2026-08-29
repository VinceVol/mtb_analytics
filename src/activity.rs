use std::path::PathBuf;

use fit::Fit;

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
            // dbg!(&m);
            match m.kind {
                //Save segment info
                fitsdk::MessageType::SegmentLap => {
                    println!("segment data!");
                    dbg!(&m.values);
                }
                fitsdk::MessageType::TimestampCorrelation => {
                    println!("Time stamp data!");
                    dbg!(&m.values);
                }
                _ => continue,
            }
        }
        //eventually return something
    }
}

mod d_test {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    #[test]
    fn test_fit() {
        Activity::new(PathBuf::from("./Data/TestSeg.fit"));
        assert_eq!(false, true);
    }
}
