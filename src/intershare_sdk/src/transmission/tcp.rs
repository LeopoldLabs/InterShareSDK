use std::sync::atomic::{AtomicBool, Ordering};
use std::io;
use std::net::SocketAddr;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use log::info;
use prost_stream::Stream;
use protocol::communication::request::RequestTypes;
use protocol::communication::Request;
use tokio::sync::RwLock;

use crate::communication::initiate_receiver_communication;
use crate::connection_request::ConnectionRequest;
use crate::nearby_server::{NearbyConnectionDelegate, InternalNearbyServer};
use crate::stream::Close;

pub struct TcpServer {
    pub port: u16,
    listener: Option<TcpListener>,
    delegate: Arc<RwLock<Box<dyn NearbyConnectionDelegate>>>,
    file_storage: String,
    running: Arc<AtomicBool>
}

impl InternalNearbyServer {
    pub(crate) async fn new_tcp_server(&self, delegate: Arc<RwLock<Box<dyn NearbyConnectionDelegate>>>, file_storage: String) -> Result<TcpServer, io::Error> {
        let addresses = [
            SocketAddr::from(([0, 0, 0, 0], 80)),
            SocketAddr::from(([0, 0, 0, 0], 8080)),
            SocketAddr::from(([0, 0, 0, 0], 0))
        ];

        let listener = TcpListener::bind(&addresses[..])?;
        listener.set_nonblocking(false).expect("Failed to set non blocking");
        let port = listener.local_addr()?.port();

        return Ok(TcpServer {
            port,
            listener: Some(listener),
            delegate,
            file_storage,
            running: Arc::new(AtomicBool::new(true))
        });
    }

    pub async fn start_loop(&self) {
        let Some(tcp_server) = &*self.tcp_server.read().await else {
            return;
        };

        let listener = tcp_server.listener.as_ref().expect("Listener is not initialized").try_clone().expect("Failed to clone listener");
        let delegate = tcp_server.delegate.clone();
        let file_storage = tcp_server.file_storage.clone();
        let running = tcp_server.running.clone();
        // let current_share_store = self.current_share_store.clone();

        tokio::spawn(async move {
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
                let transfer_request = match prost_stream.recv::<Request>() {
                    Ok(message) => message,
                    Err(error) => {
                        println!("Error {:}", error);
                        continue;
                    }
                };

                if transfer_request.r#type== RequestTypes::ShareRequest as i32 {
                    let connection_request = ConnectionRequest::new(
                        transfer_request,
                        Box::new(encrypted_stream),
                        file_storage.clone()
                    );

                    delegate.read().await.received_connection_request(Arc::new(connection_request));
                } else {
                    // NearbyServer::received_convenience_download_request(transfer_request, current_share_store.clone()).await;
                }
            }
        });
    }

    pub async fn stop_tcp_server(&self) {
        let Some(tcp_server) = &*self.tcp_server.read().await else {
            return;
        };

        tcp_server.running.store(false, Ordering::SeqCst);
        info!("TCP server stopped.");
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
