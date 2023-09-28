use dotenv::dotenv;
use tempfile::NamedTempFile;
use std::{
    fs::File,
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};
use tokio::time::sleep;

fn write_to_gpx(coords: Vec<(f64, f64)>) -> Result<(), Box<dyn std::error::Error>> {
    let file_name = "Marine Corps Marathon Route";
    let mut file = File::create("mcm.gpx").expect("Unable to create file");

    writeln!(
        file,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
            <gpx version="1.1" creator="https://onthegomap.com" xmlns="http://www.topografix.com/GPX/1/1">
              <metadata>
                <name>{}</name>
              </metadata>
              <rte>
                <name>0.84 mi route</name>"#,
        file_name
    )?;

    // Write each coordinate to the file
    for (lat, lng) in coords.iter() {
        writeln!(file, r#"    <rtept lat="{}" lon="{}"/>"#, lat, lng)
            .expect("Unable to write data");
    }

    // Close the rte and gpx tags
    writeln!(file, "  </rte>\n</gpx>").expect("Unable to write data");

    Ok(())
}

struct Tile {
    x: i32,
    y: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let mapbox_access_token =
        std::env::var("MAPBOX_ACCESS_TOKEN").expect("MAPBOX_ACCESS_TOKEN not set");
    // For further reading, see https://www.maptiler.com/google-maps-coordinates-tile-bounds-projection/#3/15.00/50.00
    // for how to calculate the tile coordinates. These were just visually
    // obtained from scrolling around and seeing the network requests.
    let top_left_tile = Tile { x: 25062, y: 18730 };
    let bottom_right_tile = Tile { x: 25080, y: 18751 };
    let z = 16;

    for x in top_left_tile.x..bottom_right_tile.x {
        for y in top_left_tile.y..bottom_right_tile.y {
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
            
            let output = Command::new("tippecanoe-decode")
                .arg(temp_file.path().to_str().unwrap())
                .arg(z.to_string())
                .arg(y.to_string())
                .arg(x.to_string())
                .output()
                .expect("Failed to run tippecanoe-decode");

            let geojson_data = String::from_utf8_lossy(&output.stdout);
            println!("the response is: {:?}", geojson_data);

            // let response: OnTheGoResponse =
            //     reqwest::get(&url).await?.json::<OnTheGoResponse>().await?;

            // // Extract the shape value for the leg (if exists)
            // if let Some(leg) = response.legs.get(0) {
            //     let encoded = &leg.shape;
            //     // Decode the shape value and accumulate
            //     all_decoded_coords.extend(decode(encoded));
            // }

            // Rate limiting
            sleep(Duration::from_secs(1)).await;
        }
    }

    // Write all the accumulated decoded coordinates to GPX
    // write_to_gpx(all_decoded_coords)?;

    Ok(())
}
