use std::{fmt::Debug, fs::File, io::{Read, Write}, path::Path, sync::Arc};
use fast_qr::convert::{image::ImageBuilder, Builder, Shape};
use fast_qr::qr::QRBuilder;
use log::{error, info};
use prost_stream::Stream;
use protocol::{communication::{request::{Intent, RequestTypes}, FileTransferIntent, ClipboardTransferIntent, Request, TransferRequestResponse}, discovery::{Device, DeviceConnectionInfo}};
use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use crate::{connection::Connection, convert_os_str, encryption::generate_secure_base64_token, errors::ConnectErrors};
use crate::nearby_server::L2CapDelegate;

pub enum ConnectionMedium {
    BLE,
    WiFi
}

pub enum SendProgressState {
    Unknown,
    Connecting,
    Requesting,
    ConnectionMediumUpdate { medium: ConnectionMedium },
    Transferring { progress: f64 },
    Cancelled,
    Finished,
    Declined
}

pub trait SendProgressDelegate: Send + Sync + Debug {
    fn progress_changed(&self, progress: SendProgressState);
}

pub struct ShareStore {
    pub request_id: String,
    pub file_paths: Option<Vec<String>>,
    pub clipboard: Option<String>,
    allow_convenience_share: bool,
    pub zip: Option<Arc<RwLock<File>>>,
    pub tmp_file: Arc<RwLock<Option<NamedTempFile>>>,
    ble_l2_cap_client: Arc<RwLock<Option<Box<dyn L2CapDelegate>>>>,
    device_connection_info: DeviceConnectionInfo
}

fn update_progress(progress_delegate: &Option<Box<dyn SendProgressDelegate>>, state: SendProgressState) {
    if let Some(progress_delegate) = progress_delegate {
        progress_delegate.progress_changed(state);
    }
}

impl ShareStore {
    #[uniffi::constructor]
    pub fn new(
            file_paths: Option<Vec<String>>,
            clipboard: Option<String>,
            allow_convenience_share: bool,
            zip: Option<Arc<RwLock<File>>>,
            tmp_file: Arc<RwLock<Option<NamedTempFile>>>,
            ble_l2_cap_client: Arc<RwLock<Option<Box<dyn L2CapDelegate>>>>,
            device_connection_info: DeviceConnectionInfo) -> Self {
        Self {
            request_id: generate_secure_base64_token(23),
            file_paths,
            clipboard,
            allow_convenience_share,
            zip,
            tmp_file,
            ble_l2_cap_client,
            device_connection_info
        }
    }

    pub async fn send_to(&self, receiver: Device, progress_delegate: Option<Box<dyn SendProgressDelegate>>) -> Result<(), ConnectErrors> {
        if self.file_paths.is_none() {
            return self.send_text(receiver, progress_delegate).await
        } else {
            return self.send_files(receiver, progress_delegate).await
        }
    }

    async fn send_text(&self, receiver: Device, progress_delegate: Option<Box<dyn SendProgressDelegate>>) -> Result<(), ConnectErrors> {
        let Some(text) = &self.clipboard else {
            return Err(ConnectErrors::NoTextProvided);
        };

        update_progress(&progress_delegate, SendProgressState::Connecting);

        let connection = Connection::new(self.ble_l2_cap_client.clone());

        let mut encrypted_stream = match connection.connect(receiver, &progress_delegate).await {
            Ok(connection) => connection,
            Err(error) => {
                update_progress(&progress_delegate, SendProgressState::Unknown);
                return Err(error)
            }
        };

        let mut proto_stream = Stream::new(&mut encrypted_stream);

        update_progress(&progress_delegate, SendProgressState::Transferring { progress: 0.0 });

        let transfer_request = Request {
            r#type: RequestTypes::ShareRequest as i32,
            device: self.device_connection_info.device.clone(),
            share_id: None,
            intent: Some(Intent::Clipboard(ClipboardTransferIntent {
                clipboard_content: text.to_string()
            }))
        };

        update_progress(&progress_delegate, SendProgressState::Transferring { progress: 0.8 });
        let _ = proto_stream.send(&transfer_request);
        update_progress(&progress_delegate, SendProgressState::Finished);

        return Ok(());
    }

    async fn send_files(&self, receiver: Device, progress_delegate: Option<Box<dyn SendProgressDelegate>>) -> Result<(), ConnectErrors> {
        let Some(file_paths) = &self.file_paths else {
            return Err(ConnectErrors::NoFilesProvided);
        };

        let Some(zip_file) = &self.zip else {
            return Err(ConnectErrors::NoFilesProvided);
        };

        let Some(ref tmp_file) = *self.tmp_file.read().await else {
            return Err(ConnectErrors::NoFilesProvided);
        };

        update_progress(&progress_delegate, SendProgressState::Connecting);

        let connection = Connection::new(self.ble_l2_cap_client.clone());

        let mut encrypted_stream = match connection.connect(receiver, &progress_delegate).await {
            Ok(connection) => connection,
            Err(error) => {
                update_progress(&progress_delegate, SendProgressState::Unknown);
                return Err(error)
            }
        };

        let mut proto_stream = Stream::new(&mut encrypted_stream);

        update_progress(&progress_delegate, SendProgressState::Requesting);

        let file_name = {
            if file_paths.len() == 1 {
                let path = Path::new(file_paths.first().unwrap());
                Some(convert_os_str(path.file_name().expect("Failed to get file name")))
            } else {
                None
            }
        };

        let file_size = zip_file.read().await.metadata()
            .expect("Failed to retrieve metadata from ZIP")
            .len();

        info!("Finished ZIP with a size of: {:?}", file_size);

        let transfer_request = Request {
            r#type: RequestTypes::ShareRequest as i32,
            device: self.device_connection_info.device.clone(),
            share_id: None,
            intent: Some(Intent::FileTransfer(FileTransferIntent {
                file_name,
                file_size,
                file_count: file_paths.len() as u64
            }))
        };

        let _ = proto_stream.send(&transfer_request);

        let response = match proto_stream.recv::<TransferRequestResponse>() {
            Ok(message) => message,
            Err(error) => return Err(ConnectErrors::FailedToGetTransferRequestResponse { error: error.to_string() })
        };

        if !response.accepted {
            update_progress(&progress_delegate, SendProgressState::Declined);
            return Err(ConnectErrors::Declined);
        }

        let mut buffer = [0; 1024];

        update_progress(&progress_delegate, SendProgressState::Transferring { progress: 0.0 });

        let mut all_written: usize = 0;

        let Ok(mut tmp_file) = tmp_file.reopen() else {
            return Err(ConnectErrors::NoFilesProvided);
        };

        while let Ok(read_size) = tmp_file.read(&mut buffer) {
            if read_size == 0 {
                break;
            }

            let written_bytes = encrypted_stream.write(&buffer[..read_size])
                .expect("Failed to write file buffer");

            if written_bytes <= 0 {
                break;
            }

            all_written += written_bytes;

            update_progress(&progress_delegate, SendProgressState::Transferring { progress: (all_written as f64 / file_size as f64) });
        }

        info!("Written {all_written} bytes");

        if (all_written as f64) < (file_size as f64) {
            update_progress(&progress_delegate, SendProgressState::Cancelled);
        } else {
            update_progress(&progress_delegate, SendProgressState::Finished);
        }

        return Ok(());
    }

    pub async fn close(&self) {
        let mut guard = self.tmp_file.write().await;

        // Take ownership of the NamedTempFile, leaving None behind
        if let Some(tmp_file) = guard.take() {
            let _ = tmp_file.close(); // You can handle the Result here if needed
        }
    }

    /// https://share.intershare.app?id=hgf8o47fdsb394mv385&ip=192.168.12.13&port=5200&device_id=9A403351-A926-4D1C-855F-432A6ED51E0E&protocol_version=1
    pub fn generate_link(&self) -> Option<String> {
        if !self.allow_convenience_share {
            return None;
        }

        let Some(device) = self.device_connection_info.device.clone() else {
            return None;
        };

        let Some(tcp_connection_info) = self.device_connection_info.tcp.clone() else {
            return None;
        };

        let ip = tcp_connection_info.hostname;
        let port = tcp_connection_info.port;

        let link = format!("https://s.intershare.app?i={0}&ip={1}&p={2}&d={3}",
            self.request_id,
            ip,
            port,
            device.id
        );

        return Some(link);
    }

    pub fn generate_qr_code(&self, dark_mode: bool) -> Option<Vec<u8>> {
        let link = self.generate_link()?;

        let qrcode = QRBuilder::new(link)
            .build()
            .unwrap();

        let img = ImageBuilder::default()
            .shape(Shape::Circle)
            .module_color(if dark_mode { [255, 255, 255, 255] } else { [0, 0, 0, 255] })
            .background_color([0, 0, 0, 0])
            .fit_width(300)
            .to_bytes(&qrcode);

        match img {
            Ok(bytes) => return Some(bytes),
            Err(error_message) => {
                error!("Error while trying to generate QR code: {:?}", error_message);
                return None;
            },
        }
    }
}
