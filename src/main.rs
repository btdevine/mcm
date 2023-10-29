use std::{fs::File, io::BufWriter};

use geo_types::Point;
use gpx::{write, Gpx, Track, TrackSegment, Waypoint};
use proj::Proj;
use serde::{Deserialize, Serialize};

// This was generated with https://app.quicktype.io/
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarineDataLayers {
    object_id_field_name: String,
    unique_id_field: UniqueIdField,
    global_id_field_name: String,
    geometry_properties: GeometryProperties,
    geometry_type: String,
    spatial_reference: SpatialReference,
    fields: Vec<Field>,
    features: Vec<Feature>,
}

#[derive(Serialize, Deserialize)]
struct Feature {
    attributes: Attributes,
    geometry: Geometry,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Attributes {
    #[serde(rename = "OBJECTID")]
    objectid: i64,
    course: String,
    #[serde(rename = "Shape__Length")]
    shape_length: f64,
}

#[derive(Serialize, Deserialize)]
struct Geometry {
    paths: Vec<Vec<[f64; 2]>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Field {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    alias: String,
    sql_type: String,
    domain: Option<serde_json::Value>,
    default_value: Option<serde_json::Value>,
    length: Option<i64>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeometryProperties {
    shape_length_field_name: String,
    units: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpatialReference {
    wkid: i64,
    latest_wkid: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniqueIdField {
    name: String,
    is_system_maintained: bool,
}

fn write_segment(
    converter: &Proj,
    segment: Vec<[f64; 2]>,
    reversed: bool,
) -> Result<Vec<Waypoint>, Box<dyn std::error::Error>> {
    let segment = if reversed {
        segment.into_iter().rev().collect()
    } else {
        segment
    };
    let mut track_points: Vec<Waypoint> = Vec::with_capacity(segment.len());

    for point in segment {
        let (lng, lat) = converter.convert((point[0], point[1]))?;
        // Create a track point with the converted coordinates
        let track_point = Waypoint::new(Point::new(lng, lat));
        track_points.push(track_point);
    }

    Ok(track_points)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://services3.arcgis.com/uriB49wQuOhO1ZVZ/arcgis/rest/\
               services/Marine_Corps_Marathon_Map_WFL1/FeatureServer/4/query?\
               where=1%3D1&outFields=*&f=json";

    let response: MarineDataLayers = reqwest::get(url).await?.json().await?;

    // Initialize a GPX object
    let mut gpx = Gpx {
        version: gpx::GpxVersion::Gpx11, // Setting the version to 1.1
        ..Default::default()
    };

    // Convert from Web Mercator to WGS84
    let from = "EPSG:3857";
    let to = "EPSG:4326";
    let converter = Proj::new_known_crs(from, to, None)?;

    let mut track = Track::default();
    let mut track_segment = TrackSegment::default();
    for feature in response.features {
        if feature.attributes.course == "MCM" {
            let paths = feature.geometry.paths;

            // Define a closure to handle segment processing
            let mut process_segment = |index: usize, reversed: bool| {
                if let Some(path) = paths.get(index) {
                    if let Ok(segment) = write_segment(&converter, path.clone(), reversed) {
                        track_segment.points.extend(segment);
                    } else {
                        eprintln!("Failed to process segment at index {}", index);
                    }
                } else {
                    eprintln!("Invalid path index: {}", index);
                }
            };

            process_segment(9, true);
            process_segment(11, false);
            process_segment(13, false);
            process_segment(14, false);
            process_segment(15, false);
            process_segment(17, false);
            process_segment(18, false);
            process_segment(20, false);
            process_segment(21, false);
            process_segment(20, true);
            process_segment(19, false);
            process_segment(17, true);
            process_segment(16, false);
            process_segment(12, false);
            process_segment(6, false);
            process_segment(8, false);
            process_segment(7, false);
            process_segment(5, false);
            process_segment(3, false);
            process_segment(1, false);
            process_segment(0, true);
            process_segment(1, true);
            process_segment(2, false);
            process_segment(4, false);
            process_segment(10, false);
        }
    }
    track.segments.push(track_segment);
    gpx.tracks.push(track);

    let file_name = "mcm.gpx";
    let file = File::create(file_name)?;
    let writer = BufWriter::new(file);
    write(&gpx, writer)?;

    println!("Done! File written to {}", file_name);

    Ok(())
}
