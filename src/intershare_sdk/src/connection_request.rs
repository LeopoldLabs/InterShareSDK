use std::fmt::Debug;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicBool, Ordering};
use log::{error, warn};
use prost_stream::Stream;
use protocol::communication::transfer_request::Intent;
use protocol::communication::{ClipboardTransferIntent, FileTransferIntent, TransferRequest, TransferRequestResponse};
use protocol::discovery::Device;
use tempfile::NamedTempFile;
use tokio::sync::RwLock;
use crate::{encryption::EncryptedReadWrite, nearby::ConnectionIntentType};
use crate::zip::unzip_file;
use crate::{BLE_BUFFER_SIZE};

pub enum ReceiveProgressState {
    Unknown,
    Handshake,
    Receiving { progress: f64 },
    Extracting,
    Cancelled,
    Finished
}

pub trait ReceiveProgressDelegate: Send + Sync + Debug {
    fn progress_changed(&self, progress: ReceiveProgressState);
}

struct SharedVariables {
    receive_progress_delegate: Option<Box<dyn ReceiveProgressDelegate>>,
}

pub struct ConnectionRequest {
    transfer_request: TransferRequest,
    connection: Arc<Mutex<Box<dyn EncryptedReadWrite>>>,
    file_storage: String,
    should_cancel: AtomicBool,
    variables: Arc<RwLock<SharedVariables>>,
    tmp_dir: Option<String>
}

impl ConnectionRequest {
    pub fn new(transfer_request: TransferRequest, connection: Box<dyn EncryptedReadWrite>, file_storage: String, tmp_dir: Option<String>) -> Self {
        Self {
            tmp_dir,
            transfer_request,
            connection: Arc::new(Mutex::new(connection)),
            file_storage,
            should_cancel: AtomicBool::new(false),
            variables: Arc::new(RwLock::new(SharedVariables {
                receive_progress_delegate: None
            }))
        }
    }

    pub fn set_progress_delegate(&self, delegate: Box<dyn ReceiveProgressDelegate>) {
        let mut variables = self.variables.blocking_write();
        variables.receive_progress_delegate = Some(delegate);
    }

    pub fn get_sender(&self) -> Device {
        self.transfer_request.device.clone().expect("Device information missing")
    }

    pub fn get_intent(&self) -> Intent {
        self.transfer_request.intent.clone().expect("Intent information missing")
    }

    pub fn get_intent_type(&self) -> ConnectionIntentType {
        match self.transfer_request.intent.clone().expect("Intent information missing") {
            Intent::FileTransfer(_) => ConnectionIntentType::FileTransfer,
            Intent::Clipboard(_) => ConnectionIntentType::FileTransfer
        }
    }

    pub fn get_file_transfer_intent(&self) -> Option<FileTransferIntent> {
        match self.transfer_request.intent.clone().expect("Intent information missing") {
            Intent::FileTransfer(file_transfer_intent) => Some(file_transfer_intent),
            Intent::Clipboard(_) => None
        }
    }

    pub fn get_clipboard_intent(&self) -> Option<ClipboardTransferIntent> {
        match self.transfer_request.intent.clone().expect("Intent information missing") {
            Intent::FileTransfer(_) => None,
            Intent::Clipboard(clipboard_intent) => Some(clipboard_intent)
        }
    }

    pub fn decline(&self) {
        if let Ok(mut connection_guard) = self.connection.lock() {
            let mut stream = Stream::new(&mut *connection_guard);

            let _ = stream.send(&TransferRequestResponse { accepted: false });
            connection_guard.close();
        }
    }

    fn update_progress(&self, new_state: ReceiveProgressState) {
        if let Some(receive_progress_delegate) = &self.variables.blocking_read().receive_progress_delegate {
            receive_progress_delegate.progress_changed(new_state);
        }
    }

    pub fn cancel(&self) {
        self.should_cancel.store(true, Ordering::Relaxed);
    }

    pub fn accept(&self) -> Option<Vec<String>> {
        self.update_progress(ReceiveProgressState::Handshake);

        if let Ok(mut connection_guard) = self.connection.lock() {
            let mut stream = Stream::new(&mut *connection_guard);

            let _ = stream.send(&TransferRequestResponse { accepted: true });

            match self.get_intent() {
                Intent::FileTransfer(file_transfer) => self.handle_file(connection_guard, file_transfer),
                Intent::Clipboard(clipboard) => self.handle_clipboard(clipboard)
            }
        } else {
            None
        }
    }

    fn handle_clipboard(&self, _clipboard_transfer_intent: ClipboardTransferIntent) -> Option<Vec<String>> {
        panic!("Not implemented yet");
    }

    fn create_tmp_file(&self) -> NamedTempFile {
        #[cfg(not(target_os="android"))]
        return NamedTempFile::new().expect("Failed to create temporary ZIP file.");

        #[cfg(target_os="android")]
        return NamedTempFile::new_in(self.tmp_dir.clone().expect("tmp dir is not set on android")).expect("Failed to create temporary ZIP file.");
    }

    fn handle_file(&self, mut stream: MutexGuard<Box<dyn EncryptedReadWrite>>, file_transfer: FileTransferIntent) -> Option<Vec<String>> {
        let named_file = self.create_tmp_file();
        let mut zip_file = named_file.reopen().expect("Failed to reopen temporary ZIP file");

        let mut buffer = [0; BLE_BUFFER_SIZE];
        let mut all_read = 0;

        while let Ok(read_size) = stream.read(&mut buffer) {
            if self.should_cancel.load(Ordering::Relaxed) || read_size == 0 {
                break;
            }

            all_read += read_size as u64;

            zip_file.write_all(&buffer[..read_size])
                .expect("Failed to write file to disk");

            let progress = all_read as f64 / file_transfer.file_size as f64;
            self.update_progress(ReceiveProgressState::Receiving { progress });

            if all_read >= file_transfer.file_size { break; }
        }

        stream.close();

        if all_read < file_transfer.file_size {
            let _ = named_file.close();
            warn!("Wrong file size. Expected: {:?}, but got {:?}", file_transfer.file_size, all_read);
            self.update_progress(ReceiveProgressState::Cancelled);
            return None;
        }

        self.update_progress(ReceiveProgressState::Extracting);
        match unzip_file(zip_file, &self.file_storage) {
            Ok(files) => {
                self.update_progress(ReceiveProgressState::Finished);
                Some(files)
            }
            Err(error) => {
                error!("Error while unzipping: {:?}", error);
                self.update_progress(ReceiveProgressState::Cancelled);
                None
            }
        }
    }
}
