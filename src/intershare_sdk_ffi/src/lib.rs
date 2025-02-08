use std::sync::Arc;

pub use intershare_sdk::{BLE_DISCOVERY_CHARACTERISTIC_UUID, BLE_SERVICE_UUID, ClipboardTransferIntent, VersionCompatibility, is_compatible};
pub use intershare_sdk::connection_request::{ConnectionRequest, ReceiveProgressState, ReceiveProgressDelegate};
pub use intershare_sdk::Device;
pub use intershare_sdk::discovery::{BleDiscoveryImplementationDelegate, Discovery};
pub use intershare_sdk::DiscoveryDelegate as DeviceListUpdateDelegate;
pub use intershare_sdk::encryption::EncryptedStream;
pub use intershare_sdk::nearby_server::{ShareProgressState, ShareProgressDelegate, BleServerImplementationDelegate, ConnectionIntentType, L2CapDelegate, NearbyConnectionDelegate, NearbyServer};
pub use intershare_sdk::protocol::communication::FileTransferIntent;
pub use intershare_sdk::stream::NativeStreamDelegate;
pub use intershare_sdk::transmission::TransmissionSetupError;
pub use intershare_sdk::errors::*;
pub use intershare_sdk::connection::handle_incoming_l2cap_connection;
pub use intershare_sdk::*;
pub use intershare_sdk::share_store::{ConnectionMedium, SendProgressState, SendProgressDelegate, ShareStore};
pub use intershare_sdk::get_log_file_path_str;
use intershare_sdk::protocol::discovery::{BluetoothLeConnectionInfo, TcpConnectionInfo};

pub mod async_code;

// #[derive(Debug, thiserror::Error)]
// pub enum ExternalIOError {
//     #[error("IO Error: {reason}")]
//     IOError { reason: String }
// }
//
// impl From<io::Error> for ExternalIOError {
//     fn from(error: io::Error) -> Self {
//         return ExternalIOError::IOError { reason: error.to_string() }
//     }
// }
//

#[cfg(target_os = "android")]
#[uniffi::export]
pub fn set_tmp_dir(tmp: String) {
    intershare_sdk::set_tmp_dir(tmp)
}

pub fn get_ble_service_uuid() -> String {
    return BLE_SERVICE_UUID.to_string();
}

pub fn get_ble_discovery_characteristic_uuid() -> String {
    return BLE_DISCOVERY_CHARACTERISTIC_UUID.to_string();
}

pub struct InternalDiscovery {
    handler: Arc<std::sync::RwLock<Discovery>>
}

impl InternalDiscovery {
    pub fn new(delegate: Option<Box<dyn DeviceListUpdateDelegate>>) -> Result<Self, DiscoverySetupError> {

        Ok(Self {
            handler: Arc::new(std::sync::RwLock::new(Discovery::new(delegate)?))
        })
    }

    pub fn get_devices(&self) -> Vec<Device> {
        return self.handler.read().expect("Failed to lock handler").get_devices()
    }

    pub fn add_ble_implementation(&self, implementation: Box<dyn BleDiscoveryImplementationDelegate>) {
        self.handler.write().expect("Failed to lock handler").add_ble_implementation(implementation);
    }

    pub fn start(&self) {
        self.handler.read().expect("Failed to lock handler").start();
    }

    pub fn stop(&self) {
        self.handler.read().expect("Failed to lock handler").stop();
    }

    pub fn parse_discovery_message(&self, data: Vec<u8>, ble_uuid: Option<String>) {
        self.handler.write().expect("Failed to lock handler").parse_discovery_message(data, ble_uuid);
    }
}

uniffi::include_scaffolding!("intershare_sdk");
