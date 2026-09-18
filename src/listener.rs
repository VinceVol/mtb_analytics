use std::io::Read;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use serde::Deserialize;
use tiny_http::{Header, Method, Response, Server};

#[derive(Debug, Clone)]
pub enum WebMessage {
    MapClick { lat: f64, lng: f64 },
    SegmentSelected(String),
    GateClicked { gate_id: usize, dataset_name: String },
    Unknown { action: String, payload: serde_json::Value },
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
            let cors_methods = Header::from_str("Access-Control-Allow-Methods: GET, POST, OPTIONS").unwrap();
            let cors_headers = Header::from_str("Access-Control-Allow-Headers: Content-Type").unwrap();

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

            // 2. Handle /api/event Endpoint
            if request.url().starts_with("/api/event") && request.method() == &Method::Post {
                let mut content = String::new();
                if request.as_reader().read_to_string(&mut content).is_ok() {
                    if let Ok(incoming) = serde_json::from_str::<IncomingPayload>(&content) {
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
                                let gate_id = incoming.data["gate_id"].as_u64().unwrap_or(0) as usize;
                                let dataset_name = incoming.data["dataset_name"]
                                    .as_str()
                                    .unwrap_or("Unknown")
                                    .to_string();

                                WebMessage::GateClicked {
                                    gate_id,
                                    dataset_name,
                                }
                            }
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
