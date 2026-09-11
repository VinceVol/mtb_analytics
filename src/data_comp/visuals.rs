use geojson::{Feature, FeatureCollection, Geometry, GeometryValue};
use serde_json::{json, Map};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::gate::Gate;

pub fn generate_track_geojson(
    data: &[(f32, f32, f32)],
    variable_name: &str,
    labels: Option<&[(f32, f32, String)]>,
    gates: Option<&[Gate]>,
) -> FeatureCollection {
    if data.is_empty() && gates.map_or(true, |g| g.is_empty()) {
        return FeatureCollection {
            bbox: None,
            features: vec![],
            foreign_members: None,
        };
    }

    let mut features = Vec::new();

    // 1. Build line segments with raw data values attached
    for window in data.windows(2) {
        let (lat1, lon1, _) = window[0];
        let (lat2, lon2, val2) = window[1];

        let line_coords = vec![
            vec![lon1 as f64, lat1 as f64],
            vec![lon2 as f64, lat2 as f64],
        ];

        let geometry = Geometry::new(GeometryValue::new_line_string(line_coords));
        let mut properties = Map::new();

        properties.insert("stroke-width".to_string(), json!(6));
        properties.insert(variable_name.to_string(), json!(val2));

        features.push(Feature {
            bbox: None,
            geometry: Some(geometry),
            id: None,
            properties: Some(properties),
            foreign_members: None,
        });
    }

    // 2. Build Point features for text markers/labels
    if let Some(label_list) = labels {
        for (lat, lon, text) in label_list {
            let point_coords = vec![*lon as f64, *lat as f64];
            let geometry = Geometry::new(GeometryValue::new_point(point_coords));

            let mut properties = Map::new();
            properties.insert("label".to_string(), json!(text));

            features.push(Feature {
                bbox: None,
                geometry: Some(geometry),
                id: None,
                properties: Some(properties),
                foreign_members: None,
            });
        }
    }

    // 3. Build features for Gates
    if let Some(gate_list) = gates {
        for (idx, gate) in gate_list.iter().enumerate() {
            let (l_lon, l_lat) = gate.left_pivot;
            let (r_lon, r_lat) = gate.right_pivot;

            let gate_coords = vec![
                vec![l_lon as f64, l_lat as f64],
                vec![r_lon as f64, r_lat as f64],
            ];
            let line_geom = Geometry::new(GeometryValue::new_line_string(gate_coords));

            let mut line_props = Map::new();
            line_props.insert("stroke".to_string(), json!("#000000"));
            line_props.insert("stroke-width".to_string(), json!(3));
            line_props.insert("is_gate".to_string(), json!(true));

            features.push(Feature {
                bbox: None,
                geometry: Some(line_geom),
                id: None,
                properties: Some(line_props),
                foreign_members: None,
            });

            let mid_lon = (l_lon + r_lon) / 2.0;
            let mid_lat = (l_lat + r_lat) / 2.0;
            let point_geom = Geometry::new(GeometryValue::new_point(vec![
                mid_lon as f64,
                mid_lat as f64,
            ]));

            let mut label_props = Map::new();
            label_props.insert("gate_label".to_string(), json!(format!("Gate {}", idx)));

            features.push(Feature {
                bbox: None,
                geometry: Some(point_geom),
                id: None,
                properties: Some(label_props),
                foreign_members: None,
            });
        }
    }

    FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    }
}

pub fn open_map_in_browser(
    geojson: &FeatureCollection,
    variable_name: &str,
    output_path: &Path,
    map_title: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let json_str = geojson.to_string();

    let raw_html = r##"<!DOCTYPE html>
<html>
<head>
    <title>{{MAP_TITLE}}</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="referrer" content="no-referrer-when-downgrade">
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <style>
        body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; }
        #map { height: 100vh; width: 100vw; }

        .control-panel {
            position: absolute;
            top: 15px;
            right: 15px;
            z-index: 1000;
            background: rgba(15, 23, 42, 0.9);
            color: #f8fafc;
            padding: 16px;
            border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.4);
            border: 1px solid #334155;
            min-width: 220px;
        }
        .control-panel h4 { margin: 0 0 12px 0; font-size: 14px; text-transform: uppercase; letter-spacing: 0.05em; }
        .slider-group { margin-bottom: 10px; }
        .slider-group label { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 4px; }
        .slider-group input[type="range"] { width: 100%; cursor: pointer; }

        .checkbox-group {
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 13px;
            font-weight: 500;
            margin-top: 12px;
            padding-top: 10px;
            border-top: 1px solid #334155;
            cursor: pointer;
        }
        .checkbox-group input { cursor: pointer; width: 16px; height: 16px; }

        .map-label-badge {
            background-color: #1e293b; color: #ffffff; padding: 4px 8px; border-radius: 6px;
            font-size: 12px; font-weight: 600; white-space: nowrap; box-shadow: 0 2px 6px rgba(0,0,0,0.3);
            border: 1px solid #475569;
        }
        .gate-label-badge {
            background-color: #0f172a; color: #f8fafc; padding: 2px 6px; border-radius: 4px;
            font-size: 10px; font-weight: 700; white-space: nowrap; box-shadow: 0 1px 4px rgba(0,0,0,0.4);
            border: 1px solid #94a3b8;
        }
    </style>
</head>
<body>
    <div id="map"></div>
    <div class="control-panel">
        <h4>Color Gradient Bounds</h4>
        <div class="slider-group">
            <label>Min ({{VAR_NAME}}): <span id="min-val-display">0</span></label>
            <input type="range" id="min-slider" step="0.1">
        </div>
        <div class="slider-group">
            <label>Max ({{VAR_NAME}}): <span id="max-val-display">0</span></label>
            <input type="range" id="max-slider" step="0.1">
        </div>
        <label class="checkbox-group">
            <input type="checkbox" id="gate-toggle" checked>
            Show Gates
        </label>
    </div>

    <script>
        const map = L.map('map');
        L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{z}/{y}/{x}', {
            maxZoom: 19,
            attribution: 'Tiles © Esri'
        }).addTo(map);

        const trackData = {{JSON_STR}};
        const varName = "{{VAR_NAME}}";

        let values = [];
        trackData.features.forEach(f => {
            if (f.properties && f.properties[varName] !== undefined) {
                values.push(f.properties[varName]);
            }
        });

        let dataMin = values.length ? Math.min(...values) : 0;
        let dataMax = values.length ? Math.max(...values) : 1;

        function lerp(a, b, t) { return a + (b - a) * Math.min(Math.max(t, 0), 1); }
        function valueToHexColor(val, minVal, maxVal) {
            let t = maxVal > minVal ? Math.min(Math.max((val - minVal) / (maxVal - minVal), 0), 1) : 0;
            let r, g, b;
            if (t < 0.5) {
                let subT = t * 2.0;
                r = lerp(0, 255, subT); g = 255; b = 0;
            } else {
                let subT = (t - 0.5) * 2.0;
                r = 255; g = lerp(255, 0, subT); b = 0;
            }
            return "#" + [r, g, b].map(x => Math.round(x).toString(16).padStart(2, "0")).join("");
        }

        const minSlider = document.getElementById('min-slider');
        const maxSlider = document.getElementById('max-slider');
        const minDisplay = document.getElementById('min-val-display');
        const maxDisplay = document.getElementById('max-val-display');
        const gateToggle = document.getElementById('gate-toggle');

        minSlider.min = dataMin; minSlider.max = dataMax; minSlider.value = dataMin;
        maxSlider.min = dataMin; maxSlider.max = dataMax; maxSlider.value = dataMax;
        minDisplay.innerText = dataMin.toFixed(1);
        maxDisplay.innerText = dataMax.toFixed(1);

        let geoJsonLayer = null;

        function updateMap() {
            const currentMin = parseFloat(minSlider.value);
            const currentMax = parseFloat(maxSlider.value);
            const showGates = gateToggle.checked;

            if (geoJsonLayer) { map.removeLayer(geoJsonLayer); }

            geoJsonLayer = L.geoJSON(trackData, {
                filter: feature => {
                    if (!showGates) {
                        const isGateLine = feature.properties && feature.properties.is_gate;
                        const isGateLabel = feature.properties && feature.properties.gate_label;
                        if (isGateLine || isGateLabel) {
                            return false;
                        }
                    }
                    return true;
                },
                style: feature => {
                    if (feature.properties.is_gate) {
                        return { color: '#000000', weight: 3, opacity: 1.0 };
                    }
                    const val = feature.properties[varName];
                    const color = val !== undefined ? valueToHexColor(val, currentMin, currentMax) : '#FF0000';
                    return { color: color, weight: feature.properties['stroke-width'] || 6, opacity: 0.9 };
                },
                pointToLayer: (feature, latlng) => {
                    if (feature.properties) {
                        if (feature.properties.label) {
                            const labelIcon = L.divIcon({
                                className: 'custom-map-label',
                                html: `<div class="map-label-badge">${feature.properties.label}</div>`,
                                iconSize: null
                            });
                            return L.marker(latlng, { icon: labelIcon });
                        }
                        if (feature.properties.gate_label) {
                            const gateIcon = L.divIcon({
                                className: 'custom-gate-label',
                                html: `<div class="gate-label-badge">${feature.properties.gate_label}</div>`,
                                iconSize: null
                            });
                            return L.marker(latlng, { icon: gateIcon });
                        }
                    }
                    return L.marker(latlng);
                },
                onEachFeature: (feature, layer) => {
                    if (feature.properties && feature.properties[varName] !== undefined) {
                        layer.bindPopup(varName + ": " + feature.properties[varName]);
                    }
                }
            }).addTo(map);
        }

        minSlider.addEventListener('input', (e) => {
            if (parseFloat(e.target.value) > parseFloat(maxSlider.value)) {
                maxSlider.value = e.target.value;
                maxDisplay.innerText = e.target.value;
            }
            minDisplay.innerText = parseFloat(e.target.value).toFixed(1);
            updateMap();
        });

        maxSlider.addEventListener('input', (e) => {
            if (parseFloat(e.target.value) < parseFloat(minSlider.value)) {
                minSlider.value = e.target.value;
                minDisplay.innerText = e.target.value;
            }
            maxDisplay.innerText = parseFloat(e.target.value).toFixed(1);
            updateMap();
        });

        gateToggle.addEventListener('change', updateMap);

        updateMap();
        map.fitBounds(geoJsonLayer.getBounds());
    </script>
</body>
</html>"##;

    let html_content = raw_html
        .replace("{{MAP_TITLE}}", map_title)
        .replace("{{VAR_NAME}}", variable_name)
        .replace("{{JSON_STR}}", &json_str);

    let mut file = File::create(output_path)?;
    file.write_all(html_content.as_bytes())?;

    opener::open(output_path)?;

    Ok(())
}

#[cfg(test)]
mod d_test {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_geo_graph() {
        println!("Generating test GPS track data...");

        let mock_track = vec![
            (37.7749, -122.4194, 5.0),
            (37.7752, -122.4185, 18.0),
            (37.7758, -122.4172, 35.0),
            (37.7766, -122.4158, 55.0),
            (37.7775, -122.4141, 72.0),
            (37.7783, -122.4128, 40.0),
            (37.7791, -122.4115, 8.0),
        ];

        let mock_labels = vec![
            (37.7749, -122.4194, "Start Line".to_string()),
            (37.7775, -122.4141, "Max Speed: 72.0".to_string()),
            (37.7791, -122.4115, "Finish Line".to_string()),
        ];

        // Gates defined as (lon, lat)
        let mock_gates = vec![
            Gate {
                left_pivot: (-122.4196, 37.7748),
                right_pivot: (-122.4192, 37.7750),
            },
            Gate {
                left_pivot: (-122.4143, 37.7774),
                right_pivot: (-122.4139, 37.7776),
            },
        ];

        let var_name = "speed";

        let geojson =
            generate_track_geojson(&mock_track, var_name, Some(&mock_labels), Some(&mock_gates));

        let output_file = Path::new("test_map.html");
        let result = open_map_in_browser(&geojson, var_name, output_file, "TESTMAP");

        assert!(result.is_ok(), "Failed to create or open map file");
        assert!(output_file.exists(), "HTML map file was not saved to disk");

        // 6 line segments + 3 labels + (2 gates * 2 features each) = 13 features
        assert_eq!(geojson.features.len(), 13);
    }
}
