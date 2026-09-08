use geojson::{Feature, FeatureCollection, Geometry, GeometryValue};
use serde_json::{Map, json};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::segments::Gate;

/// Interpolates linearly between `a` and `b` by fraction `t`
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Converts a scaled numeric value into a Hex color string on a Green -> Yellow -> Red gradient.
fn value_to_hex_color(value: f32, min_val: f32, max_val: f32) -> String {
    let t = if max_val > min_val {
        ((value - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let (r, g, b) = if t < 0.5 {
        let sub_t = t * 2.0;
        (lerp(0.0, 255.0, sub_t), 255.0, 0.0)
    } else {
        let sub_t = (t - 0.5) * 2.0;
        (255.0, lerp(255.0, 0.0, sub_t), 0.0)
    };

    format!("#{:02X}{:02X}{:02X}", r as u8, g as u8, b as u8)
}

/// Converts track data, point labels, and gates into a GeoJSON FeatureCollection.
///
/// - `data`: Slice of tuples `(lat, lon, variable_value)`
/// - `variable_name`: Property key for the data variable (e.g. "speed")
/// - `labels`: Optional slice of tuples `(lat, lon, label_string)` for custom map pins
/// - `gates`: Optional slice of `Gate` structs where pivots are `(lon, lat)`
/// - `custom_min_max`: Optional manually specified `(min, max)` bounds
pub fn generate_track_geojson(
    data: &[(f32, f32, f32)],
    variable_name: &str,
    labels: Option<&[(f32, f32, String)]>,
    gates: Option<&[Gate]>,
    custom_min_max: Option<(f32, f32)>,
) -> FeatureCollection {
    if data.is_empty() && gates.map_or(true, |g| g.is_empty()) {
        return FeatureCollection {
            bbox: None,
            features: vec![],
            foreign_members: None,
        };
    }

    let (min_val, max_val) = custom_min_max.unwrap_or_else(|| {
        if data.is_empty() {
            (0.0, 1.0)
        } else {
            let min = data.iter().map(|p| p.2).fold(f32::INFINITY, f32::min);
            let max = data.iter().map(|p| p.2).fold(f32::NEG_INFINITY, f32::max);
            (min, max)
        }
    });

    let mut features = Vec::new();

    // 1. Build line segments for the colored track
    for window in data.windows(2) {
        let (lat1, lon1, _) = window[0];
        let (lat2, lon2, val2) = window[1];

        let line_coords = vec![
            vec![lon1 as f64, lat1 as f64],
            vec![lon2 as f64, lat2 as f64],
        ];

        let geometry = Geometry::new(GeometryValue::new_line_string(line_coords));
        let mut properties = Map::new();

        let hex_color = value_to_hex_color(val2, min_val, max_val);
        properties.insert("stroke".to_string(), json!(hex_color));
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

    // 3. Build features for Gates (Black lines with index badges at the midpoint)
    if let Some(gate_list) = gates {
        for (idx, gate) in gate_list.iter().enumerate() {
            let (l_lon, l_lat) = gate.left_pivot;
            let (r_lon, r_lat) = gate.right_pivot;

            // Gate line geometry
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

            // Midpoint label marker
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
) -> Result<(), Box<dyn std::error::Error>> {
    let json_str = geojson.to_string();

    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>GPS Map Track</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="referrer" content="no-referrer-when-downgrade">
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <style>
        body {{ margin: 0; padding: 0; }}
        #map {{ height: 100vh; width: 100vw; }}
        
        /* Custom clean text badge styling */
        .map-label-badge {{
            background-color: #1e293b;
            color: #ffffff;
            padding: 4px 8px;
            border-radius: 6px;
            font-family: system-ui, -apple-system, sans-serif;
            font-size: 12px;
            font-weight: 600;
            white-space: nowrap;
            box-shadow: 0 2px 6px rgba(0,0,0,0.3);
            border: 1px solid #475569;
        }}

        /* Compact styling for gate index markers */
        .gate-label-badge {{
            background-color: #0f172a;
            color: #f8fafc;
            padding: 2px 6px;
            border-radius: 4px;
            font-family: system-ui, -apple-system, sans-serif;
            font-size: 10px;
            font-weight: 700;
            white-space: nowrap;
            box-shadow: 0 1px 4px rgba(0,0,0,0.4);
            border: 1px solid #94a3b8;
        }}
    </style>
</head>
<body>
    <div id="map"></div>
    <script>
        const map = L.map('map');

        L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Topo_Map/MapServer/tile/{{z}}/{{y}}/{{x}}', {{
            maxZoom: 19,
            attribution: 'Tiles © Esri'
        }}).addTo(map);

        const trackData = {json_str};
        const layer = L.geoJSON(trackData, {{
            style: feature => ({{
                color: feature.properties.stroke || '#FF0000',
                weight: feature.properties['stroke-width'] || 6,
                opacity: 0.9
            }}),
            pointToLayer: (feature, latlng) => {{
                if (feature.properties) {{
                    if (feature.properties.label) {{
                        const labelIcon = L.divIcon({{
                            className: 'custom-map-label',
                            html: `<div class="map-label-badge">${{feature.properties.label}}</div>`,
                            iconSize: null
                        }});
                        return L.marker(latlng, {{ icon: labelIcon }});
                    }}
                    if (feature.properties.gate_label) {{
                        const gateIcon = L.divIcon({{
                            className: 'custom-gate-label',
                            html: `<div class="gate-label-badge">${{feature.properties.gate_label}}</div>`,
                            iconSize: null
                        }});
                        return L.marker(latlng, {{ icon: gateIcon }});
                    }}
                }}
                return L.marker(latlng);
            }},
            onEachFeature: (feature, layer) => {{
                if (feature.properties && feature.properties["{var_name}"] !== undefined) {{
                    layer.bindPopup("{var_name}: " + feature.properties["{var_name}"]);
                }}
            }}
        }}).addTo(map);

        map.fitBounds(layer.getBounds());
    </script>
</body>
</html>"#,
        json_str = json_str,
        var_name = variable_name
    );

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

        let geojson = generate_track_geojson(
            &mock_track,
            var_name,
            Some(&mock_labels),
            Some(&mock_gates),
            None,
        );

        let output_file = Path::new("test_map.html");
        let result = open_map_in_browser(&geojson, var_name, output_file);

        assert!(result.is_ok(), "Failed to create or open map file");
        assert!(output_file.exists(), "HTML map file was not saved to disk");

        // 6 line segments + 3 labels + (2 gates * 2 features each) = 13 features
        assert_eq!(geojson.features.len(), 13);
    }
}
