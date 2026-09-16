use geojson::{Feature, FeatureCollection, Geometry, GeometryValue};
use serde_json::{Map, json};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::gate::Gate;

/// Calculates horizontal distance between two points in meters using Haversine formula
fn haversine_distance_m(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    let r = 6371000.0; // Earth radius in meters
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

pub fn generate_track_geojson(
    data: &[(f32, f32, f32, f32)], //lat,lon,slope,var
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
        let (lat1, lon1, _, _) = window[0];
        let (lat2, lon2, slope, val2) = window[1];

        let line_coords = vec![
            vec![lon1 as f64, lat1 as f64],
            vec![lon2 as f64, lat2 as f64],
        ];

        let geometry = Geometry::new(GeometryValue::new_line_string(line_coords));
        let mut properties = Map::new();

        properties.insert("stroke-width".to_string(), json!(6));
        properties.insert(variable_name.to_string(), json!(val2));
        properties.insert("slope".to_string(), json!(slope));

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
    datasets: &[(FeatureCollection, &str, &str)],
    output_path: &Path,
    map_title: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if datasets.is_empty() {
        return Err("No datasets provided to visualize.".into());
    }

    let mut data_map = Map::new();
    let mut default_plot_key = "";

    for (idx, (geojson, var_name, plot_name)) in datasets.iter().enumerate() {
        if idx == 0 {
            default_plot_key = plot_name;
        }

        let dataset_payload = json!({
            "plot_name": plot_name,
            "var_name": var_name,
            "geojson": geojson,
        });

        data_map.insert(plot_name.to_string(), dataset_payload);
    }

    let all_data_json = serde_json::to_string(&data_map)?;

    let raw_html = r##"<!DOCTYPE html>
<html>
<head>
    <title>{{MAP_TITLE}}</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <style>
        body { margin: 0; padding: 0; font-family: system-ui, -apple-system, sans-serif; }
        #map { height: 100vh; width: 100vw; }

        .control-panel {
            position: absolute;
            top: 15px; right: 15px; z-index: 1000;
            background: rgba(15, 23, 42, 0.92); color: #f8fafc;
            padding: 16px; border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.4); border: 1px solid #334155;
            min-width: 250px;
        }
        .control-panel h4 { margin: 0 0 12px 0; font-size: 14px; text-transform: uppercase; letter-spacing: 0.05em; }
        .section-header { font-size: 11px; font-weight: 700; color: #94a3b8; text-transform: uppercase; margin: 12px 0 6px 0; border-top: 1px solid #334155; padding-top: 8px; }
        .select-group { margin-bottom: 10px; }
        .select-group label { display: block; font-size: 12px; margin-bottom: 4px; }
        .select-group select {
            width: 100%; padding: 6px; border-radius: 4px;
            background: #1e293b; color: #f8fafc; border: 1px solid #475569; font-size: 13px;
        }
        .slider-group { margin-bottom: 8px; }
        .slider-group label { display: flex; justify-content: space-between; font-size: 12px; margin-bottom: 2px; }
        .slider-group input[type="range"] { width: 100%; cursor: pointer; }

        .checkbox-group {
            display: flex; align-items: center; gap: 8px;
            font-size: 13px; font-weight: 500; margin-top: 12px;
            padding-top: 10px; border-top: 1px solid #334155; cursor: pointer;
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
        <h4>MTB Data View</h4>
        
        <div class="select-group">
            <label for="plot-select">Select Dataset:</label>
            <select id="plot-select"></select>
        </div>

        <div class="select-group">
            <label for="property-select">Color By Property:</label>
            <select id="property-select"></select>
        </div>

        <div class="select-group">
            <label for="slope-filter">Slope Category:</label>
            <select id="slope-filter">
                <option value="all" selected>All Terrain</option>
                <option value="uphill">Uphill (> 1%)</option>
                <option value="flat">Flat (-1% to 1%)</option>
                <option value="downhill">Downhill (< -1%)</option>
            </select>
        </div>

        <div class="section-header">Value Range Filter</div>
        <div class="slider-group">
            <label>Min (<span id="var-name-min"></span>): <span id="min-val-display">0</span></label>
            <input type="range" id="min-slider" step="0.1">
        </div>
        <div class="slider-group">
            <label>Max (<span id="var-name-max"></span>): <span id="max-val-display">0</span></label>
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
            maxZoom: 19, attribution: 'Tiles © Esri'
        }).addTo(map);

        const allDatasets = {{ALL_DATA_JSON}};
        let currentPlotKey = "{{DEFAULT_PLOT_KEY}}";
        let activePropKey = "";

        const plotSelect = document.getElementById('plot-select');
        const propertySelect = document.getElementById('property-select');
        const slopeSelect = document.getElementById('slope-filter');
        const minSlider = document.getElementById('min-slider');
        const maxSlider = document.getElementById('max-slider');
        const minDisplay = document.getElementById('min-val-display');
        const maxDisplay = document.getElementById('max-val-display');
        const varNameMinDisplay = document.getElementById('var-name-min');
        const varNameMaxDisplay = document.getElementById('var-name-max');
        const gateToggle = document.getElementById('gate-toggle');

        Object.keys(allDatasets).forEach(plotName => {
            const opt = document.createElement('option');
            opt.value = plotName; opt.innerText = plotName;
            if (plotName === currentPlotKey) opt.selected = true;
            plotSelect.appendChild(opt);
        });

        function getFeatureValue(props, key) {
            if (!props || !key) return undefined;
            if (props[key] !== undefined) return props[key];
            const matchingKey = Object.keys(props).find(k => k.trim() === key.trim());
            return matchingKey !== undefined ? props[matchingKey] : undefined;
        }

        function populatePropertyDropdown(datasetPayload) {
            propertySelect.innerHTML = "";
            const keysFound = new Set();
            const defaultVar = datasetPayload.var_name;

            if (datasetPayload.geojson && datasetPayload.geojson.features) {
                datasetPayload.geojson.features.forEach(f => {
                    if (f.properties) {
                        Object.keys(f.properties).forEach(k => {
                            if (k !== 'is_gate' && k !== 'gate_label' && k !== 'label' && k !== 'stroke-width') {
                                keysFound.add(k);
                            }
                        });
                    }
                });
            }

            let foundDefault = false;
            keysFound.forEach(key => {
                const opt = document.createElement('option');
                opt.value = key; opt.innerText = key;
                if (key === defaultVar) { opt.selected = true; foundDefault = true; }
                propertySelect.appendChild(opt);
            });

            activePropKey = foundDefault ? defaultVar : (propertySelect.value || defaultVar);
        }

        function lerp(a, b, t) { return a + (b - a) * Math.min(Math.max(t, 0), 1); }
        function valueToHexColor(val, minVal, maxVal) {
            let t = maxVal > minVal ? Math.min(Math.max((val - minVal) / (maxVal - minVal), 0), 1) : 0;
            let r, g, b;
            if (t < 0.5) {
                let subT = t * 2.0; r = lerp(0, 255, subT); g = 255; b = 0;
            } else {
                let subT = (t - 0.5) * 2.0; r = 255; g = lerp(255, 0, subT); b = 0;
            }
            return "#" + [r, g, b].map(x => Math.round(x).toString(16).padStart(2, "0")).join("");
        }

        let geoJsonLayer = null;

        function updateBoundsAndMap(resetBounds = false) {
            const datasetPayload = allDatasets[currentPlotKey];
            const trackData = datasetPayload.geojson;

            varNameMinDisplay.innerText = activePropKey;
            varNameMaxDisplay.innerText = activePropKey;

            let values = [];
            if (trackData && trackData.features) {
                trackData.features.forEach(f => {
                    const val = getFeatureValue(f.properties, activePropKey);
                    if (val !== undefined && val !== null) { values.push(Number(val)); }
                });
            }

            let dataMin = values.length ? Math.min(...values) : 0;
            let dataMax = values.length ? Math.max(...values) : 1;

            if (resetBounds) {
                minSlider.min = dataMin; minSlider.max = dataMax; minSlider.value = dataMin;
                maxSlider.min = dataMin; maxSlider.max = dataMax; maxSlider.value = dataMax;
                minDisplay.innerText = dataMin.toFixed(1);
                maxDisplay.innerText = dataMax.toFixed(1);
            }

            renderLayer(datasetPayload);
        }

        function renderLayer(datasetPayload) {
            const currentMin = parseFloat(minSlider.value);
            const currentMax = parseFloat(maxSlider.value);
            const slopeMode = slopeSelect.value;
            const showGates = gateToggle.checked;
            const trackData = datasetPayload.geojson;

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
                    if (feature.properties && feature.properties.is_gate) {
                        return { color: '#000000', weight: 4, opacity: 1.0 };
                    }

                    const val = getFeatureValue(feature.properties, activePropKey);
                    const slopeVal = getFeatureValue(feature.properties, "slope");

                    let isWithinSlope = true;
                    if (slopeVal !== undefined && slopeVal !== null && slopeMode !== "all") {
                        let s = Number(slopeVal);
                        if (slopeMode === "uphill" && s <= 1.0) {
                            isWithinSlope = false;
                        } else if (slopeMode === "flat" && (s < -1.0 || s > 1.0)) {
                            isWithinSlope = false;
                        } else if (slopeMode === "downhill" && s >= -1.0) {
                            isWithinSlope = false;
                        }
                    }

                    const targetOpacity = isWithinSlope ? 0.9 : 0.2;
                    const targetWeight = isWithinSlope ? 6 : 2;

                    const color = (val !== undefined && val !== null) 
                        ? valueToHexColor(Number(val), currentMin, currentMax) 
                        : '#FF0000';

                    return { 
                        color: color, 
                        weight: targetWeight, 
                        opacity: targetOpacity 
                    };
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
                    const val = getFeatureValue(feature.properties, activePropKey);
                    const slopeVal = getFeatureValue(feature.properties, "slope");
                    if (val !== undefined && val !== null) {
                        let popupText = `${datasetPayload.plot_name} (${activePropKey}): ${Number(val).toFixed(2)}`;
                        if (slopeVal !== undefined) {
                            popupText += `<br>Slope: ${Number(slopeVal).toFixed(1)}%`;
                        }
                        layer.bindPopup(popupText);
                    }
                }
            }).addTo(map);
        }

        // Event listeners
        plotSelect.addEventListener('change', (e) => {
            currentPlotKey = e.target.value;
            populatePropertyDropdown(allDatasets[currentPlotKey]);
            updateBoundsAndMap(true);
        });

        propertySelect.addEventListener('change', (e) => {
            activePropKey = e.target.value;
            updateBoundsAndMap(true);
        });

        slopeSelect.addEventListener('change', () => {
            renderLayer(allDatasets[currentPlotKey]);
        });

        minSlider.addEventListener('input', (e) => {
            if (parseFloat(e.target.value) > parseFloat(maxSlider.value)) {
                maxSlider.value = e.target.value;
                maxDisplay.innerText = e.target.value;
            }
            minDisplay.innerText = parseFloat(e.target.value).toFixed(1);
            renderLayer(allDatasets[currentPlotKey]);
        });

        maxSlider.addEventListener('input', (e) => {
            if (parseFloat(e.target.value) < parseFloat(minSlider.value)) {
                minSlider.value = e.target.value;
                minDisplay.innerText = e.target.value;
            }
            maxDisplay.innerText = parseFloat(e.target.value).toFixed(1);
            renderLayer(allDatasets[currentPlotKey]);
        });

        gateToggle.addEventListener('change', () => renderLayer(allDatasets[currentPlotKey]));

        populatePropertyDropdown(allDatasets[currentPlotKey]);
        updateBoundsAndMap(true);
        if (geoJsonLayer && geoJsonLayer.getBounds().isValid()) {
            map.fitBounds(geoJsonLayer.getBounds());
        }
    </script>
</body>
</html>"##;

    let html_content = raw_html
        .replace("{{MAP_TITLE}}", map_title)
        .replace("{{DEFAULT_PLOT_KEY}}", default_plot_key)
        .replace("{{ALL_DATA_JSON}}", &all_data_json);

    let mut file = File::create(output_path)?;
    file.write_all(html_content.as_bytes())?;

    opener::open(output_path)?;

    Ok(())
}