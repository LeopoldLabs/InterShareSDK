use std::ffi::OsStr;
use std::{fs, panic};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Once;

// Only Android
#[cfg(target_os="android")]
use android_logger::Config;
#[cfg(target_os="android")]
use log::LevelFilter;

// If not Android
#[cfg(not(target_os="android"))]
use simplelog::{Config, WriteLogger};
#[cfg(not(target_os="android"))]
use log::{error, info, LevelFilter};
#[cfg(not(target_os="android"))]
use directories::{BaseDirs, ProjectDirs};


pub use protocol;
pub use protocol::communication::ClipboardTransferIntent;
pub use protocol::discovery::Device;
pub use protocol::DiscoveryDelegate;

pub mod discovery;
pub mod encryption;
pub mod stream;
pub mod nearby;
pub mod transmission;
pub mod communication;
pub mod connection_request;
pub mod errors;
mod zip;

pub const BLE_SERVICE_UUID: &str = "68D60EB2-8AAA-4D72-8851-BD6D64E169B7";
pub const BLE_DISCOVERY_CHARACTERISTIC_UUID: &str = "0BEBF3FE-9A5E-4ED1-8157-76281B3F0DA5";
pub const BLE_WRITE_CHARACTERISTIC_UUID: &str = "8B1C476F-1197-4A0D-A484-378AABE85317";
pub const BLE_BUFFER_SIZE: usize = 1024;

static INIT_LOGGER: Once = Once::new();

fn convert_os_str(os_str: &OsStr) -> Option<String> {
    os_str.to_str().map(|s| s.to_string())
}

#[cfg(not(target_os="android"))]
fn get_log_file_path() -> Option<PathBuf> {
    let project_dirs = BaseDirs::new()?;
    let config_dir = project_dirs.config_dir();

    return Some(config_dir
        .join("InterShare")
        .join( "intershare.log")
    )
}

#[cfg(target_os="android")]
pub fn init_logger() {
    android_logger::init_once(
        Config::default().with_max_level(LevelFilter::Trace),
    );
}

#[cfg(not(target_os="android"))]
fn set_panic_logger() {
    panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location().unwrap();
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic message".to_string()
        };

        error!(
            "Panic occurred at file '{}' line {}: {}",
            location.file(),
            location.line(),
            message
        );
    }));
}

#[cfg(not(target_os="android"))]
pub fn init_logger() {
    INIT_LOGGER.call_once(|| {
        // Get the platform-specific configuration folder
        let log_file_path = get_log_file_path().expect("Failed to get log file path");

        // Ensure the directory exists
        if let Some(parent) = log_file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create log directory");
        }

        // Initialize the logger
        let log_file = File::create(log_file_path).expect("Failed to create log file");
        WriteLogger::init(LevelFilter::Info, Config::default(), log_file)
            .expect("Failed to initialize logger");

        set_panic_logger();


        info!("Logger initialized successfully.");
    });
}
