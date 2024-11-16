use crate::ble::ble_server::BleServer;
use intershare_sdk::nearby::{NearbyConnectionDelegate, SendProgressDelegate};
use intershare_sdk::nearby::NearbyServer as InternalNearbyServer;
use intershare_sdk::{init_logger, Device};
use std::sync::Arc;
use dirs::download_dir;
use tokio::runtime::Runtime;
use intershare_sdk::errors::ConnectErrors;
use uniffi::deps::log::info;

pub struct NearbyServer {
    runtime: Runtime,
    internal_nearby_server: Arc<InternalNearbyServer>
}

impl NearbyServer {
    pub fn new(my_device: Device, delegate: Option<Box<dyn NearbyConnectionDelegate>>) -> NearbyServer {
        init_logger();
        let downloads_dir = download_dir().expect("Failed to get downloads directory").to_string_lossy().to_string();
        info!("Downloads directory: {}", downloads_dir);

        let nearby = Arc::new(InternalNearbyServer::new(my_device, downloads_dir, delegate, None));
        let ble_server = BleServer::new(Arc::clone(&nearby))
            .expect("Failed to initialize BLE Server");

        nearby.add_bluetooth_implementation(Box::new(ble_server));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        Self {
            runtime,
            internal_nearby_server: nearby
        }
    }

    pub fn start(&self) {
        self.runtime.block_on(self.internal_nearby_server.start());
    }

    pub fn stop(&self) {
        self.internal_nearby_server.stop()
    }

    pub fn restart_server(&self) {
        self.runtime.block_on(self.internal_nearby_server.restart_server());
    }

    pub fn send_files(&self, receiver: Device, file_paths: Vec<String>, progress_delegate: Option<Box<dyn SendProgressDelegate>>) -> Result<(), ConnectErrors> {
        return self.runtime.block_on(self.internal_nearby_server.send_files(receiver, file_paths, progress_delegate))
    }
}
