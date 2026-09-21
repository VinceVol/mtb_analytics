//Basically we dont want to reinvent the wheel here. We want to port functions from FFmpeg by legit
// formatting and executing terminal commands

use chrono::Utc;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use rkyv::{Archive, Deserialize, Serialize, deserialize, rancor};
use serde_json::Value;
use std::fs::File;
use std::path::Path;
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
        server_port: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(gv_1.time_vec.len(), gv_2.time_vec.len());
        assert!(!gv_1.time_vec.is_empty());
        if start_gate == gv_1.time_vec.len() {
            start_gate -= 1;
        }

        let clip1_output = "./clip_1.mp4";
        let clip2_output = "./clip_2.mp4";
        let _ = std::fs::remove_file(clip1_output);
        let _ = std::fs::remove_file(clip2_output);

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

        let off_1: i32 = gv_1.time_vec[0].unwrap_or_default() as i32 - gv_1_video.start_time as i32;
        let off_2: i32 = gv_2.time_vec[0].unwrap_or_default() as i32 - gv_2_video.start_time as i32;

        let vid_start_1 = (unix_start_1 as i32 - gv_1_video.start_time as i32 - off_1) as u32;
        let vid_duration_1 = unix_end_1 - unix_start_1;

        let vid_start_2 = (unix_start_2 as i32 - gv_2_video.start_time as i32 - off_2) as u32;
        let vid_duration_2 = unix_end_2 - unix_start_2;

        let comparison_duration = (vid_duration_1.max(vid_duration_2)) as f64;

        // 1. Generate Clip 1 with web-compatible H.264 video codec
        let filter_spec_1 = format!(
            "[0:v]trim=start={vid_start_1}:duration={vid_duration_1},setpts=PTS-STARTPTS,scale=-1:1080,format=yuv420p[v1]; \
             [0:a]atrim=start={vid_start_1}:duration={vid_duration_1},asetpts=PTS-STARTPTS[a1]"
        );
        let mut child_1 = FfmpegCommand::new()
            .input(gv_1_video.fp.clone())
            .filter_complex(filter_spec_1)
            .map("[v1]")
            .map("[a1]")
            .args(["-c:v", "libx264", "-preset", "ultrafast"])
            .output(clip1_output)
            .spawn()?;

        for event in child_1.iter()? {
            if let FfmpegEvent::Progress(progress) = event {
                println!("Processing Clip 1 frame: {}", progress.frame);
            }
        }

        // 2. Generate Clip 2 with web-compatible H.264 video codec
        let filter_spec_2 = format!(
            "[0:v]trim=start={vid_start_2}:duration={vid_duration_2},setpts=PTS-STARTPTS,scale=-1:1080,format=yuv420p[v2]; \
             [0:a]atrim=start={vid_start_2}:duration={vid_duration_2},asetpts=PTS-STARTPTS[a2]"
        );
        let mut child_2 = FfmpegCommand::new()
            .input(gv_2_video.fp.clone())
            .filter_complex(filter_spec_2)
            .map("[v2]")
            .map("[a2]")
            .args(["-c:v", "libx264", "-preset", "ultrafast"])
            .output(clip2_output)
            .spawn()?;

        for event in child_2.iter()? {
            if let FfmpegEvent::Progress(progress) = event {
                println!("Processing Clip 2 frame: {}", progress.frame);
            }
        }

        let page_title = format!(
            "Gates {}-{} ({} vs {})",
            start_gate,
            start_gate + 1,
            chrono::DateTime::<Utc>::from_timestamp(unix_start_1.into(), 0)
                .unwrap_or_default()
                .format("%m/%d/%Y"),
            chrono::DateTime::<Utc>::from_timestamp(unix_start_2.into(), 0)
                .unwrap_or_default()
                .format("%m/%d/%Y")
        );

        let html_output_path = Path::new("./video_aligner.html");

        open_video_aligner_in_browser(
            Path::new(clip1_output),
            Path::new(clip2_output),
            0.0,
            0.0,
            comparison_duration,
            html_output_path,
            &page_title,
            server_port,
        )?;

        Ok(())
    }
}

pub fn open_video_aligner_in_browser(
    vid1_path: &Path,
    vid2_path: &Path,
    initial_start_1: f64,
    initial_start_2: f64,
    duration: f64,
    output_html_path: &Path,
    page_title: &str,
    server_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    // Convert MP4 binary files into Base64 Data URIs to bypass browser file:// CORS restrictions
    use base64::Engine;

    let vid1_bytes = std::fs::read(vid1_path)?;
    let vid2_bytes = std::fs::read(vid2_path)?;

    let vid1_b64 = base64::engine::general_purpose::STANDARD.encode(&vid1_bytes);
    let vid2_b64 = base64::engine::general_purpose::STANDARD.encode(&vid2_bytes);

    let vid1_url = format!("data:video/mp4;base64,{}", vid1_b64);
    let vid2_url = format!("data:video/mp4;base64,{}", vid2_b64);

    let payload = serde_json::json!({
        "vid1_url": vid1_url,
        "vid2_url": vid2_url,
        "vid1_start": initial_start_1,
        "vid2_start": initial_start_2,
        "duration": duration,
    });

    let data_json = serde_json::to_string(&payload)?;

    let listener_script = format!(
        r#"
        <script>
            function sendToRust(action, payload) {{
                fetch('http://127.0.0.1:{server_port}/api/event', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ action: action, data: payload }})
                }}).catch(err => console.error('Failed to send event to Rust:', err));
            }}

            window.addEventListener('beforeunload', function () {{
                const url = 'http://127.0.0.1:{server_port}/api/event';
                const payload = JSON.stringify({{ action: 'tab_closed', data: {{}} }});

                if (navigator.sendBeacon) {{
                    const blob = new Blob([payload], {{ type: 'text/plain;charset=UTF-8' }});
                    navigator.sendBeacon(url, blob);
                }} else {{
                    fetch(url, {{
                        method: 'POST',
                        headers: {{ 'Content-Type': 'text/plain' }},
                        body: payload,
                        keepalive: true
                    }}).catch(() => {{}});
                }}
            }});
        </script>
        "#
    );

    let raw_html = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>__PAGE_TITLE__</title>
    __LISTENER_SCRIPT__
    <style>
        :root {
            --bg-color: #0f172a;
            --panel-bg: #1e293b;
            --border-color: #334155;
            --accent-color: #38bdf8;
            --text-main: #f8fafc;
            --text-sub: #94a3b8;
        }

        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: var(--bg-color);
            color: var(--text-main);
            height: 100vh;
            display: flex;
            flex-direction: column;
            overflow: hidden;
        }

        .toolbar {
            background: var(--panel-bg);
            border-bottom: 1px solid var(--border-color);
            padding: 12px 20px;
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 16px;
            z-index: 10;
        }

        .title {
            font-size: 16px;
            font-weight: 700;
            letter-spacing: 0.05em;
            color: var(--accent-color);
            text-transform: uppercase;
        }

        .playback-controls {
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .btn {
            background: #334155;
            color: var(--text-main);
            border: 1px solid #475569;
            padding: 6px 14px;
            border-radius: 6px;
            font-weight: 600;
            font-size: 13px;
            cursor: pointer;
            transition: background 0.15s ease;
        }
        .btn:hover { background: #475569; }
        .btn-primary { background: #0284c7; border-color: #38bdf8; }
        .btn-primary:hover { background: #0369a1; }

        .time-display {
            font-family: monospace;
            font-size: 14px;
            background: #0f172a;
            padding: 4px 10px;
            border-radius: 4px;
            border: 1px solid var(--border-color);
        }

        .offset-controls {
            display: flex;
            align-items: center;
            gap: 8px;
            background: #0f172a;
            padding: 6px 12px;
            border-radius: 6px;
            border: 1px solid var(--border-color);
        }

        .offset-label { font-size: 12px; font-weight: 600; color: var(--text-sub); }
        .offset-val { font-family: monospace; font-size: 14px; color: #f59e0b; font-weight: bold; width: 70px; text-align: center; }

        .video-container {
            flex: 1;
            display: flex;
            width: 100%;
            height: calc(100vh - 120px);
            background: #000;
        }

        .video-wrapper {
            flex: 1;
            position: relative;
            display: flex;
            align-items: center;
            justify-content: center;
            border-right: 1px solid var(--border-color);
            background: #000;
        }
        .video-wrapper:last-child { border-right: none; }

        video {
            width: 100%;
            height: 100%;
            object-fit: contain;
        }

        .video-badge {
            position: absolute;
            top: 12px;
            left: 12px;
            background: rgba(15, 23, 42, 0.85);
            border: 1px solid var(--border-color);
            padding: 4px 8px;
            border-radius: 4px;
            font-size: 11px;
            font-weight: 700;
            color: var(--text-sub);
            letter-spacing: 0.05em;
        }

        .timeline-bar {
            background: var(--panel-bg);
            border-top: 1px solid var(--border-color);
            padding: 10px 20px;
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .timeline-slider {
            flex: 1;
            cursor: pointer;
            height: 6px;
            accent-color: var(--accent-color);
        }
    </style>
</head>
<body>
    <div class="toolbar">
        <div class="title">__PAGE_TITLE__</div>
        
        <div class="playback-controls">
            <button id="play-btn" class="btn btn-primary">Play</button>
            <button id="restart-btn" class="btn">Restart</button>
            <div class="time-display" id="time-display">00:00.000</div>
        </div>

        <div class="offset-controls">
            <span class="offset-label">CLIP 2 OFFSET:</span>
            <button class="btn" id="off-m1000">-1s</button>
            <button class="btn" id="off-m100">-100ms</button>
            <span class="offset-val" id="offset-display">0 ms</span>
            <button class="btn" id="off-p100">+100ms</button>
            <button class="btn" id="off-p1000">+1s</button>
        </div>

        <button id="sync-rust-btn" class="btn btn-primary">Confirm Alignment</button>
    </div>

    <div class="video-container">
        <div class="video-wrapper">
            <span class="video-badge">CLIP 1 (REFERENCE)</span>
            <video id="v1" playsinline preload="auto"></video>
        </div>
        <div class="video-wrapper">
            <span class="video-badge">CLIP 2 (ALIGNED)</span>
            <video id="v2" playsinline preload="auto"></video>
        </div>
    </div>

    <div class="timeline-bar">
        <input type="range" id="seeker" class="timeline-slider" min="0" max="100" value="0" step="0.001">
    </div>

    <script>
        const DATA = __DATA_JSON__;

        const v1 = document.getElementById('v1');
        const v2 = document.getElementById('v2');
        const playBtn = document.getElementById('play-btn');
        const restartBtn = document.getElementById('restart-btn');
        const seeker = document.getElementById('seeker');
        const timeDisplay = document.getElementById('time-display');
        const offsetDisplay = document.getElementById('offset-display');
        const confirmBtn = document.getElementById('sync-rust-btn');

        let offsetMs = 0;
        let isPlaying = false;
        let animationFrameId = null;

        // Initialize sources
        v1.src = DATA.vid1_url;
        v2.src = DATA.vid2_url;
        v1.load();
        v2.load();

        function updatePlayhead() {
            if (!v1.paused && !v1.ended) {
                const relativeTime = v1.currentTime - DATA.vid1_start;
                const progress = Math.min(Math.max(relativeTime / DATA.duration, 0), 1);
                seeker.value = progress * 100;
                
                formatTime(relativeTime);
                syncClip2Position();
                animationFrameId = requestAnimationFrame(updatePlayhead);
            }
        }

        function syncClip2Position() {
            const relTime1 = v1.currentTime - DATA.vid1_start;
            const targetV2Time = DATA.vid2_start + relTime1 + (offsetMs / 1000.0);
            
            if (Math.abs(v2.currentTime - targetV2Time) > 0.04) {
                v2.currentTime = targetV2Time;
            }
        }

        function applyOffset(deltaMs) {
            offsetMs += deltaMs;
            offsetDisplay.innerText = (offsetMs >= 0 ? "+" : "") + offsetMs + " ms";
            syncClip2Position();

            sendToRust('offset_changed', {
                offset_ms: offsetMs,
                effective_start_1: DATA.vid1_start,
                effective_start_2: DATA.vid2_start + (offsetMs / 1000.0)
            });
        }

        function formatTime(seconds) {
            const pad = (n, z = 2) => ('00' + n).slice(-z);
            const s = Math.max(0, seconds);
            const mins = Math.floor(s / 60);
            const secs = Math.floor(s % 60);
            const ms = Math.floor((s % 1) * 1000);
            timeDisplay.innerText = `${pad(mins)}:${pad(secs)}.${pad(ms, 3)}`;
        }

        playBtn.addEventListener('click', () => {
            if (isPlaying) {
                v1.pause();
                v2.pause();
                playBtn.innerText = "Play";
                isPlaying = false;
                if (animationFrameId) cancelAnimationFrame(animationFrameId);
            } else {
                syncClip2Position();
                Promise.all([v1.play(), v2.play()]).then(() => {
                    playBtn.innerText = "Pause";
                    isPlaying = true;
                    animationFrameId = requestAnimationFrame(updatePlayhead);
                }).catch(err => console.error("Playback error:", err));
            }
        });

        restartBtn.addEventListener('click', () => {
            v1.currentTime = DATA.vid1_start;
            syncClip2Position();
            seeker.value = 0;
            formatTime(0);
        });

        seeker.addEventListener('input', (e) => {
            const pct = parseFloat(e.target.value) / 100.0;
            const relTime = pct * DATA.duration;
            v1.currentTime = DATA.vid1_start + relTime;
            syncClip2Position();
            formatTime(relTime);
        });

        document.getElementById('off-m1000').addEventListener('click', () => applyOffset(-1000));
        document.getElementById('off-m100').addEventListener('click', () => applyOffset(-100));
        document.getElementById('off-p100').addEventListener('click', () => applyOffset(100));
        document.getElementById('off-p1000').addEventListener('click', () => applyOffset(1000));

        confirmBtn.addEventListener('click', () => {
            sendToRust('offset_confirmed', {
                offset_ms: offsetMs,
                effective_start_1: DATA.vid1_start,
                effective_start_2: DATA.vid2_start + (offsetMs / 1000.0)
            });
        });

        v1.addEventListener('loadedmetadata', () => {
            v1.currentTime = DATA.vid1_start;
            v2.currentTime = DATA.vid2_start;
        });
    </script>
</body>
</html>"##;

    let html_content = raw_html
        .replace("__PAGE_TITLE__", page_title)
        .replace("__LISTENER_SCRIPT__", &listener_script)
        .replace("__DATA_JSON__", &data_json);

    {
        let mut file = File::create(output_html_path)?;
        file.write_all(html_content.as_bytes())?;
        file.flush()?;
    }

    opener::open(output_html_path)?;

    Ok(())
}
