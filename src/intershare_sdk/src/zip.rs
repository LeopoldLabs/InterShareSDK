use std::{fs::File, io::BufReader, path::Path};
use log::info;

use zip::ZipArchive;

use crate::convert_os_str;

pub fn unzip_file(zip_file: File, destination: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Open the zip file
    info!("Reading ZIP file");
    let mut archive = ZipArchive::new(BufReader::new(zip_file))?;
    let mut written_files = vec![];


    info!("Unzipping...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out_path = Path::new(destination).join(file.name());
        written_files.push(convert_os_str(out_path.clone().as_os_str()).expect("Failed to convert file path OS string to string"));

        if file.name().ends_with('/') {
            info!("Creating directory: {:?}", out_path);
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.exists() {
                    info!("Creating parent directory: {:?}", parent);
                    std::fs::create_dir_all(parent)?;
                }
            }

            info!("Writing to file: {:?}", out_path);
            let mut outfile = File::create(&out_path)?;
            std::io::copy(&mut file, &mut outfile)?;
        }

        info!("Extracted file to {:?}", out_path);
    }

    Ok(written_files)
}
