use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use intershare_sdk::connection_request::ConnectionRequest;
use intershare_sdk::Device;
use intershare_sdk::nearby::NearbyConnectionDelegate;
use intershare_windows::nearby_server::NearbyServer;

#[derive(Debug)]
struct ConnectionDelegate {}

impl NearbyConnectionDelegate for ConnectionDelegate {
    fn received_connection_request(&self, request: Arc<ConnectionRequest>) {
        todo!()
    }
}

#[test]
pub fn test_new() {
    let device = Device {
        id: "37791916-4200-4cf6-b21e-8628e03bd4c5".to_string(),
        name: "Windows PC".to_string(),
        device_type: 0,
    };

    let server = NearbyServer::new(device, Some(Box::new(ConnectionDelegate { })));
    server.start();

    loop {
        sleep(Duration::from_secs(10))
    }
}
