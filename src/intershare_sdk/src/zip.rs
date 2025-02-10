use std::{fs, fs::File, io::BufReader, path::Path};
use std::collections::HashMap;
use std::path::PathBuf;
use log::{error, info};

use walkdir::WalkDir;
use zip::{ZipArchive, ZipWriter};
use zip::write::SimpleFileOptions;

use crate::convert_os_str;
use crate::nearby_server::{ShareProgressDelegate, ShareProgressState};

fn normalize_path(path: &Path) -> String {
    // Convert the path to a string using to_string_lossy()
    // and replace platform-specific separators (`\` on Windows) with `/`
    let path_str = path.to_string_lossy();
    path_str.replace(std::path::MAIN_SEPARATOR, "/")
}

pub struct CompressionProgress<'a> {
    pub total_file_count: usize,
    pub finished_files: usize,
    progress_delegate: &'a Box<dyn ShareProgressDelegate>
}

impl<'a> CompressionProgress<'a> {
    pub fn new(total_file_count: usize, finished_files: usize, progress_delegate: &'a Box<dyn ShareProgressDelegate>) -> Self {
        Self {
            total_file_count,
            finished_files,
            progress_delegate
        }
    }

    pub fn advance(&mut self) {
        self.finished_files += 1;
        let progress = self.finished_files as f64 / self.total_file_count as f64;
        info!("Progress: {:?}", progress);
        self.progress_delegate.progress_changed(ShareProgressState::Compressing { progress });
    }
}

fn get_file_count(file_paths: &Vec<String>) -> usize {
    let mut count: usize = 0;

    for path in file_paths {
        count += WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .count();
    }

    return count;
}

pub fn zip_files(tmp_file: File, file_paths: &Vec<String>, progress_delegate: &Option<Box<dyn ShareProgressDelegate>>) -> File {
    let mut zip = ZipWriter::new(tmp_file);

    let mut progress = if let Some(progress_delegate) = progress_delegate {
        let total_file_count: usize = get_file_count(&file_paths);
        Some(CompressionProgress::new(total_file_count, 0, progress_delegate))
    } else {
        None
    };

    // if let Some(progress) = &mut progress {
    //     progress.advance();
    // }

    for file_path in file_paths {
        let file = Path::new(file_path);

        if file.is_dir() {
            let prefix = file.file_name().unwrap().to_string_lossy().to_string();
            zip_directory(&mut zip, file, file, Some(&prefix), &mut progress);
        } else {
            info!("Compressing file: {:?}", file);
            zip.start_file(convert_os_str(file.file_name().unwrap()), SimpleFileOptions::default())
                .unwrap();

            let mut file = File::open(file_path).unwrap();
            let _ = std::io::copy(&mut file, &mut zip);

            if let Some(progress) = &mut progress {
                progress.advance();
            }
        }
    }

    info!("Finished compressing.");

    return zip.finish().expect("Failed to finish the ZIP");
}

pub fn zip_directory(zip: &mut ZipWriter<File>, base_dir: &Path, current_dir: &Path, prefix: Option<&str>, progress_delegate: &mut Option<CompressionProgress>) {
    // Calculate the relative path based on the base directory
    let relative_path = current_dir.strip_prefix(base_dir).unwrap_or(current_dir);
    let relative_path_str = if let Some(prefix) = prefix {
        normalize_path(&Path::new(prefix).join(relative_path))
    } else {
        normalize_path(relative_path)
    };

    info!("Zipping directory: {:?}", relative_path_str);

    // Create the directory in the ZIP archive
    if let Err(error) = zip.add_directory(&relative_path_str, SimpleFileOptions::default()) {
        error!("Error while trying to create ZIP directory: {:?}", error);
        return;
    }

    // Iterate through the directory entries
    for entry in fs::read_dir(current_dir).expect("Failed to read directory.") {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                error!("Failed to get entry: {:?}", e);
                continue;
            }
        };

        let entry_path = entry.path();

        if entry_path.is_dir() {
            // Recursively zip subdirectories
            zip_directory(zip, base_dir, &entry_path, prefix, progress_delegate);
        } else {
            // Get the relative file path and normalize it
            let file_name = entry_path.strip_prefix(base_dir).unwrap_or(&entry_path);
            let zip_file_name = if let Some(prefix) = prefix {
                normalize_path(&Path::new(prefix).join(file_name))
            } else {
                normalize_path(file_name)
            };

            info!("Adding file to ZIP: {:?}", zip_file_name);

            // Add the file to the ZIP archive
            if let Err(error) = zip.start_file(&zip_file_name, SimpleFileOptions::default()) {
                error!("Failed to start file in ZIP: {:?}", error);
                continue;
            }

            // Copy the file contents to the ZIP archive
            let mut file = match File::open(&entry_path) {
                Ok(f) => f,
                Err(e) => {
                    error!("Failed to open file {:?}: {:?}", entry_path, e);
                    continue;
                }
            };

            if let Err(error) = std::io::copy(&mut file, zip) {
                error!("Failed to copy file {:?} to ZIP: {:?}", entry_path, error);
            }

            if let Some(progress) = progress_delegate {
                progress.advance();
            }
        }
    }
}

fn get_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let mut counter = 1;
    let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().map(|ext| ext.to_string_lossy()).unwrap_or_default();

    loop {
        let new_file_name = if extension.is_empty() {
            format!("{} ({})", file_stem, counter)
        } else {
            format!("{} ({}).{}", file_stem, counter, extension)
        };

        let new_path = path.with_file_name(new_file_name);

        if !new_path.exists() {
            return new_path;
        }

        counter += 1;
    }
}

pub fn unzip_file(zip_file: File, destination: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    info!("Reading ZIP file");
    let mut archive = ZipArchive::new(BufReader::new(zip_file))?;
    let mut written_files = vec![];
    let mut directory_map = HashMap::<PathBuf, PathBuf>::new();

    info!("Unzipping...");
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let mut out_path = Path::new(destination).join(file.name());

        // Check if the parent directory has been renamed
        for (original_dir, unique_dir) in &directory_map {
            if out_path.starts_with(original_dir) {
                // Replace the original directory path with the unique directory path
                out_path = unique_dir.join(out_path.strip_prefix(original_dir).unwrap());
                break;
            }
        }

        // If this is a directory, get a unique path for it
        if file.name().ends_with('/') {
            if out_path.exists() {
                let unique_out_path = get_unique_path(&out_path);
                info!("Renaming directory {:?} to {:?}", out_path, unique_out_path);
                directory_map.insert(out_path.clone(), unique_out_path.clone());
                out_path = unique_out_path;
            }

            info!("Creating directory: {:?}", out_path);
            fs::create_dir_all(&out_path)?;
        } else {
            // Ensure the parent directory exists, checking the directory map for renamed directories
            if let Some(parent) = out_path.parent() {
                let unique_parent = directory_map.get(parent).cloned().unwrap_or_else(|| parent.to_path_buf());
                if !unique_parent.exists() {
                    info!("Creating parent directory: {:?}", unique_parent);
                    fs::create_dir_all(&unique_parent)?;
                }
            }

            // Get a unique path for the file if it already exists
            out_path = get_unique_path(&out_path);

            info!("Writing file: {:?}", out_path);
            let mut outfile = File::create(&out_path)?;
            std::io::copy(&mut file, &mut outfile)?;
        }

        info!("Extracted {:?}", out_path);
        written_files.push(out_path.to_string_lossy().to_string());
    }

    Ok(written_files)
}
