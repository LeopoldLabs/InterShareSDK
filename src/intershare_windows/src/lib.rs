#[cfg(target_os="windows")]
mod ble;
#[cfg(target_os="windows")]
pub mod nearby_server;
#[cfg(target_os="windows")]
pub mod discovery;
#[cfg(target_os="windows")]
pub use intershare_sdk::{ClipboardTransferIntent};
#[cfg(target_os="windows")]
pub use intershare_sdk::connection_request::{ConnectionRequest, ReceiveProgressState, ReceiveProgressDelegate};
#[cfg(target_os="windows")]
pub use intershare_sdk::Device;
#[cfg(target_os="windows")]
pub use intershare_sdk::DiscoveryDelegate;
#[cfg(target_os="windows")]
pub use intershare_sdk::encryption::EncryptedStream;
#[cfg(target_os="windows")]
pub use intershare_sdk::nearby::{ConnectionMedium, SendProgressState, SendProgressDelegate, BleServerImplementationDelegate, L2CapDelegate, NearbyConnectionDelegate};
#[cfg(target_os="windows")]
pub use intershare_sdk::nearby::ConnectionIntentType;
#[cfg(target_os="windows")]
pub use intershare_sdk::protocol::communication::FileTransferIntent;
#[cfg(target_os="windows")]
pub use intershare_sdk::stream::NativeStreamDelegate;
#[cfg(target_os="windows")]
pub use intershare_sdk::transmission::TransmissionSetupError;
#[cfg(target_os="windows")]
pub use intershare_sdk::errors::*;
#[cfg(target_os="windows")]
pub use intershare_sdk::*;
#[cfg(target_os="windows")]
pub use crate::discovery::{Discovery};
#[cfg(target_os="windows")]
pub use crate::nearby_server::{NearbyServer};
#[cfg(target_os="windows")]
pub use intershare_sdk::protocol::discovery::{BluetoothLeConnectionInfo, TcpConnectionInfo};

#[cfg(target_os="windows")]
uniffi::include_scaffolding!("intershare_sdk");