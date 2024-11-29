use std::sync::atomic::{AtomicBool, Ordering};
use std::{io, thread};
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use log::info;
use prost_stream::Stream;
use protocol::communication::TransferRequest;

use crate::communication::initiate_receiver_communication;
use crate::connection_request::ConnectionRequest;
use crate::nearby::NearbyConnectionDelegate;
use crate::stream::Close;

pub struct TcpServer {
    pub port: u16,
    listener: Option<TcpListener>,
    delegate: Arc<Mutex<Box<dyn NearbyConnectionDelegate>>>,
    file_storage: String,
    tmp_dir: Option<String>,
    running: Arc<AtomicBool>
}

impl TcpServer {
    pub(crate) async fn new(delegate: Arc<Mutex<Box<dyn NearbyConnectionDelegate>>>, file_storage: String, tmp_dir: Option<String>) -> Result<TcpServer, io::Error> {
        let addresses = [
            SocketAddr::from(([0, 0, 0, 0], 80)),
            SocketAddr::from(([0, 0, 0, 0], 8080)),
            SocketAddr::from(([0, 0, 0, 0], 0))
        ];

        let listener = TcpListener::bind(&addresses[..])?;
        listener.set_nonblocking(false).expect("Failed to set non blocking");
        let port = listener.local_addr()?.port();

        return Ok(Self {
            port,
            listener: Some(listener),
            delegate,
            file_storage,
            tmp_dir,
            running: Arc::new(AtomicBool::new(true))
        });
    }

    pub fn start_loop(&self) {
        let listener = self.listener.as_ref().expect("Listener is not initialized").try_clone().expect("Failed to clone listener");
        let delegate = self.delegate.clone();
        let file_storage = self.file_storage.clone();
        let tmp_dir = self.tmp_dir.clone();
        let running = self.running.clone();

        thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                let Ok((tcp_stream, _socket_address)) = listener.accept() else {
                    continue
                };

                let mut encrypted_stream = match initiate_receiver_communication(tcp_stream) {
                    Ok(request) => request,
                    Err(error) => {
                        println!("Encryption error {:}", error);
                        continue;
                    }
                };

                let mut prost_stream = Stream::new(&mut encrypted_stream);
                let transfer_request = match prost_stream.recv::<TransferRequest>() {
                    Ok(message) => message,
                    Err(error) => {
                        println!("Error {:}", error);
                        continue;
                    }
                };

                let connection_request = ConnectionRequest::new(
                    transfer_request,
                    Box::new(encrypted_stream),
                    file_storage.clone(),
                    tmp_dir.clone()
                );

                delegate.lock().expect("Failed to lock").received_connection_request(Arc::new(connection_request));
            }
        });
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = drop(self.listener.as_ref());
        info!("TCP server stopped.");
    }

    pub fn restart(&mut self) -> Result<(), io::Error> {
        info!("TCP server restarting...");
        self.stop();

        let addresses = [
            SocketAddr::from(([0, 0, 0, 0], 80)),
            SocketAddr::from(([0, 0, 0, 0], 8080)),
            SocketAddr::from(([0, 0, 0, 0], 0)),
        ];

        let listener = TcpListener::bind(&addresses[..])?;
        listener.set_nonblocking(false).expect("Failed to set non blocking");
        self.port = listener.local_addr()?.port();
        self.listener = Some(listener);
        self.running.store(true, Ordering::SeqCst);
        self.start_loop();
        info!("TCP server restarted.");
        Ok(())
    }
}

pub struct TcpClient {
}

impl TcpClient {
    pub fn connect(address: SocketAddr) -> Result<TcpStream, io::Error> {
        let std_stream = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        std_stream.set_nonblocking(false).expect("Failed to set non blocking");

        return Ok(std_stream);
    }
}

impl Close for TcpStream {
    fn close(&self) {
        // Do nothing. TCPStream closes automatically.
    }
}
