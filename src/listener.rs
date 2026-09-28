use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};

#[derive(Debug, Clone)]
pub enum WebMessage {
    MapClick {
        lat: f64,
        lng: f64,
    },
    SegmentSelected(String),
    GateClicked {
        gate_id: usize,
        dataset_name: String,
    },
    TabClosed,
    Unknown {
        action: String,
        payload: serde_json::Value,
    },
}

#[derive(Deserialize)]
struct IncomingPayload {
    action: String,
    #[serde(default)]
    data: serde_json::Value,
}

pub fn start_http_listener(server_port: u16) -> Receiver<WebMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let address = format!("127.0.0.1:{}", server_port);
        let server = Server::http(&address).expect("Failed to bind HTTP server");

        for mut request in server.incoming_requests() {
            // Standard CORS Headers for local HTTP requests from browser scripts
            let cors_origin = Header::from_str("Access-Control-Allow-Origin: *").unwrap();
            let cors_methods =
                Header::from_str("Access-Control-Allow-Methods: GET, POST, OPTIONS").unwrap();
            let cors_headers =
                Header::from_str("Access-Control-Allow-Headers: Content-Type, Range").unwrap();

            // 1. Handle CORS Preflight Requests
            if request.method() == &Method::Options {
                let response = Response::from_string("")
                    .with_status_code(200)
                    .with_header(cors_origin)
                    .with_header(cors_methods)
                    .with_header(cors_headers);
                let _ = request.respond(response);
                continue;
            }

            let url = request.url().to_string();

            // 2. Handle /api/event Endpoint
            if url.starts_with("/api/event") && request.method() == &Method::Post {
                let mut content = String::new();
                if request.as_reader().read_to_string(&mut content).is_ok() {
                    let clean_content = content.trim_matches('\0').trim();

                    if let Ok(incoming) = serde_json::from_str::<IncomingPayload>(clean_content) {
                        let msg = match incoming.action.as_str() {
                            "map_click" => {
                                let lat = incoming.data["lat"].as_f64().unwrap_or(0.0);
                                let lng = incoming.data["lng"].as_f64().unwrap_or(0.0);
                                WebMessage::MapClick { lat, lng }
                            }
                            "segment_selected" => {
                                let seg_id = incoming.data.as_str().unwrap_or_default().to_string();
                                WebMessage::SegmentSelected(seg_id)
                            }
                            "gate_clicked" => {
                                let gate_id =
                                    incoming.data["gate_id"].as_u64().unwrap_or(0) as usize;
                                let dataset_name = incoming.data["dataset_name"]
                                    .as_str()
                                    .unwrap_or("Unknown")
                                    .to_string();

                                WebMessage::GateClicked {
                                    gate_id,
                                    dataset_name,
                                }
                            }
                            "tab_closed" => WebMessage::TabClosed,
                            _ => WebMessage::Unknown {
                                action: incoming.action,
                                payload: incoming.data,
                            },
                        };

                        let _ = tx.send(msg);
                    }
                }

                let response = Response::from_string(r#"{"status":"ok"}"#)
                    .with_status_code(200)
                    .with_header(Header::from_str("Content-Type: application/json").unwrap())
                    .with_header(cors_origin)
                    .with_header(cors_methods)
                    .with_header(cors_headers);

                let _ = request.respond(response);
            }
            // 3. Handle /stream Endpoint for Media Playback
            else if url.starts_with("/stream") && request.method() == &Method::Get {
                // Parse query parameter: /stream?path=/absolute/path/to/video.mp4
                let query_str = url.split('?').nth(1).unwrap_or("");
                let raw_path = query_str
                    .split('&')
                    .find(|p| p.starts_with("path="))
                    .map(|p| p.trim_start_matches("path="))
                    .unwrap_or("");

                // Decode percent-encoded characters (e.g. %20 -> space)
                let decoded_path = match urlencoding::decode(raw_path) {
                    Ok(p) => p.into_owned(),
                    Err(_) => raw_path.to_string(),
                };

                let path = Path::new(&decoded_path);

                if !path.exists() || !path.is_file() {
                    let response = Response::from_string("File Not Found")
                        .with_status_code(404)
                        .with_header(cors_origin);
                    let _ = request.respond(response);
                    continue;
                }

                let mut file = match File::open(path) {
                    Ok(f) => f,
                    Err(_) => {
                        let response = Response::from_string("Internal Server Error")
                            .with_status_code(500)
                            .with_header(cors_origin);
                        let _ = request.respond(response);
                        continue;
                    }
                };

                let file_len = match file.metadata() {
                    Ok(meta) => meta.len(),
                    Err(_) => {
                        let response = Response::from_string("Metadata Error")
                            .with_status_code(500)
                            .with_header(cors_origin);
                        let _ = request.respond(response);
                        continue;
                    }
                };

                // Parse standard "Range: bytes=X-Y" header sent by browser media engines
                let range_header = request
                    .headers()
                    .iter()
                    .find(|h| h.field.as_str().to_string().to_lowercase() == "range")
                    .map(|h| h.value.as_str().to_string());

                let (start, end) = parse_range_header(range_header.as_deref(), file_len);
                let chunk_size = (end - start + 1) as usize;

                if file.seek(SeekFrom::Start(start)).is_err() {
                    let response = Response::from_string("Seek Error")
                        .with_status_code(500)
                        .with_header(cors_origin);
                    let _ = request.respond(response);
                    continue;
                }

                let mut buffer = vec![0u8; chunk_size];
                if file.read_exact(&mut buffer).is_err() {
                    let response = Response::from_string("Read Error")
                        .with_status_code(500)
                        .with_header(cors_origin);
                    let _ = request.respond(response);
                    continue;
                }

                // Construct HTTP 206 Partial Content response
                let mut response = Response::from_data(buffer).with_status_code(StatusCode(206));

                let content_type = if decoded_path.ends_with(".webm") {
                    "video/webm"
                } else {
                    "video/mp4"
                };

                response.add_header(
                    Header::from_str(&format!("Content-Type: {}", content_type)).unwrap(),
                );
                response.add_header(Header::from_str("Accept-Ranges: bytes").unwrap());
                response.add_header(
                    Header::from_str(&format!(
                        "Content-Range: bytes {}-{}/{}",
                        start, end, file_len
                    ))
                    .unwrap(),
                );
                response.add_header(cors_origin);
                response.add_header(cors_methods);
                response.add_header(cors_headers);

                let _ = request.respond(response);
            } else {
                let response = Response::from_string("Not Found")
                    .with_status_code(404)
                    .with_header(cors_origin);
                let _ = request.respond(response);
            }
        }
    });

    rx
}

/// Helper function to extract (start, end) byte offsets from a "Range: bytes=start-end" header.
fn parse_range_header(range: Option<&str>, file_len: u64) -> (u64, u64) {
    if file_len == 0 {
        return (0, 0);
    }

    if let Some(r) = range {
        if let Some(bytes_str) = r.strip_prefix("bytes=") {
            let parts: Vec<&str> = bytes_str.split('-').collect();
            let start = parts[0].parse::<u64>().unwrap_or(0);

            // Chunk response size to 2MB bursts if no end is specified to keep memory usage low
            let default_end = std::cmp::min(start + 2 * 1024 * 1024 - 1, file_len - 1);

            let end = parts
                .get(1)
                .filter(|s| !s.is_empty())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(default_end);

            return (start, std::cmp::min(end, file_len - 1));
        }
    }

    // Default to serving the first 2MB chunk if no range header is provided
    let end = std::cmp::min(2 * 1024 * 1024 - 1, file_len - 1);
    (0, end)
}
