use dotenv::dotenv;
use geo_types::Point;
use gpx::{write, Gpx, Track, TrackSegment, Waypoint};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::BufWriter;
use std::{fs::File, io::Write, process::Command, time::Duration};
use tempfile::NamedTempFile;
use tokio::time::sleep;

fn write_to_gpx(unique_coords: BTreeMap<i64, Vec<f64>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut waypoints = Vec::new();

    for (_, coord) in unique_coords {
        for i in (0..coord.len()).step_by(2) {
            let waypoint = Waypoint::new(Point::new(coord[i], coord[i + 1]));
            waypoints.push(waypoint);
        }
    }

    let track_segment = TrackSegment { points: waypoints };
    let track = Track {
        segments: vec![track_segment],
        ..Default::default()
    };

    let gpx = Gpx {
        tracks: vec![track],
        version: gpx::GpxVersion::Gpx11,
        ..Default::default()
    };

    // Write the GPX structure to a file
    let file = File::create("output.gpx")?;
    let writer = BufWriter::new(file);
    write(&gpx, writer)?;

    Ok(())
}

struct Tile {
    x: i32,
    y: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeoData {
    #[serde(rename = "type")]
    data_type: String,
    properties: DataProperties,
    features: Vec<FeatureCollection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeatureCollection {
    #[serde(rename = "type")]
    collection_type: String,
    properties: CollectionProperties,
    features: Vec<GeoFeature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeoFeature {
    #[serde(rename = "type")]
    feature_type: FeatureType,
    id: i64,
    properties: FeatureProperties,
    geometry: Geometry,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FeatureType {
    Feature,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Geometry {
    #[serde(rename = "type")]
    geometry_type: GeometryType,
    coordinates: GeometryCoordinate,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GeometryCoordinate {
    Single(f64),
    Array(Vec<CoordinateValue>),
    DoubleArray(Vec<Vec<CoordinateValue>>),
    TripleArray(Vec<Vec<Vec<CoordinateValue>>>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CoordinateValue {
    Single(f64),
    Array(Vec<f64>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GeometryType {
    LineString,
    MultiPolygon,
    Polygon,
    MultiPoint,
    Point,
    MultiLineString,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct FeatureProperties {
    class: Option<String>,
    #[serde(rename = "ele")]
    elevation: Option<i64>,
    index: Option<i64>,
    altitude_mode: Option<String>,
    begin: Option<String>,
    descriptio: Option<String>,
    end: Option<String>,
    name: Option<String>,
    timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionProperties {
    layer: String,
    version: i64,
    extent: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataProperties {
    zoom: i64,
    x: i64,
    y: i64,
    compressed: Option<bool>,
}

impl GeometryCoordinate {
    pub fn flatten(&self) -> Vec<f64> {
        match self {
            GeometryCoordinate::Single(val) => vec![*val],
            GeometryCoordinate::Array(arr) => arr.iter().flat_map(|v| v.flatten()).collect(),
            GeometryCoordinate::DoubleArray(arr) => arr
                .iter()
                .flat_map(|inner_arr| {
                    inner_arr
                        .iter()
                        .flat_map(|v| v.flatten())
                        .collect::<Vec<f64>>()
                })
                .collect(),
            GeometryCoordinate::TripleArray(arr) => arr
                .iter()
                .flat_map(|inner_arr| {
                    inner_arr
                        .iter()
                        .flat_map(|second_arr| {
                            second_arr
                                .iter()
                                .flat_map(|v| v.flatten())
                                .collect::<Vec<f64>>()
                        })
                        .collect::<Vec<f64>>()
                })
                .collect(),
        }
    }
}

impl CoordinateValue {
    pub fn flatten(&self) -> Vec<f64> {
        match self {
            CoordinateValue::Single(val) => vec![*val],
            CoordinateValue::Array(arr) => arr.clone(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let mapbox_access_token =
        std::env::var("MAPBOX_ACCESS_TOKEN").expect("MAPBOX_ACCESS_TOKEN not set");
    // For further reading, see https://www.maptiler.com/google-maps-coordinates-tile-bounds-projection/#3/15.00/50.00
    // for how to calculate the tile coordinates. These were just visually
    // obtained from scrolling around and seeing the network requests.
    let top_left_tile = Tile { x: 12531, y: 9365 };
    let bottom_right_tile = Tile { x: 12540, y: 9376 };
    let z = 15;

    let total_tile_count =
        (bottom_right_tile.x - top_left_tile.x + 1) * (bottom_right_tile.y - top_left_tile.y + 1);

    let mut processed_tile_count = 0;

    const NOTIFY_EVERY_PERCENT: u8 = 10;

    let mut unique_coords: BTreeMap<i64, Vec<f64>> = BTreeMap::new();

    for x in top_left_tile.x..=bottom_right_tile.x {
        for y in top_left_tile.y..=bottom_right_tile.y {
            let url = format!(
                "https://api.mapbox.com/v4/mapbox.mapbox-terrain-v2,mapbox.\
                mapbox-streets-v7,geocentric.24kvp202,geocentric.0rz5vmpj,\
                geocentric.1tryr4je/{z}/{y}/{x}.vector.pbf?sku=101U7kfJrad7a\
                &access_token={mapbox_access_token}",
            );

            let response = reqwest::get(&url).await?.bytes().await?;

            // The data is returned as a protobuf (.pbf), so we need to decode it

            // Create a temporary file
            let mut temp_file = NamedTempFile::new()?;
            temp_file.write_all(&response)?;

            // Requires tippecanoe-decode to be installed (e.g with `brew install tippecanoe`)
            let output = Command::new("tippecanoe-decode")
                .arg(temp_file.path().to_str().unwrap())
                .arg(z.to_string())
                .arg(y.to_string())
                .arg(x.to_string())
                .output()
                .expect("Failed to run tippecanoe-decode");

            let geojson_data = String::from_utf8_lossy(&output.stdout);

            let geo_data: GeoData =
                serde_json::from_str(&geojson_data).expect("Failed to parse data");

            for feature_collection in &geo_data.features {
                if feature_collection.properties.layer != "mcm-2018-marathon-v1-31s1ih" {
                    continue;
                }
                println!("mcm data found for tile {}, {}", x, y);
                for feature in &feature_collection.features {
                    let flattened_coords = feature.geometry.coordinates.flatten();
                    if let Some(existing_coords) = unique_coords.get_mut(&feature.id) {
                        // Add the new coordinates that don't already exist in `existing_coords`
                        for i in (0..flattened_coords.len()).step_by(2) {
                            let new_coord = (flattened_coords[i], flattened_coords[i + 1]);
                            if !existing_coords
                                .windows(2)
                                .any(|window| window == [new_coord.0, new_coord.1])
                            {
                                existing_coords.push(new_coord.0);
                                existing_coords.push(new_coord.1);
                            }
                        }
                    } else {
                        // The feature.id doesn't exist, so we insert it normally
                        unique_coords.insert(feature.id, flattened_coords);
                    }
                }
            }

            processed_tile_count += 1;
            let divisor = total_tile_count / NOTIFY_EVERY_PERCENT as i32;
            if processed_tile_count % (divisor) == 0 {
                println!(
                    "Processed {}% of the features.",
                    10 * (processed_tile_count / divisor)
                );
            }

            // Rate limiting
            sleep(Duration::from_millis(100)).await;
        }
    }

    // Write all the accumulated decoded coordinates to GPX
    write_to_gpx(unique_coords)?;

    println!("Done!");

    Ok(())
}
