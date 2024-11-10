use std::thread::sleep;
use std::time::Duration;
use intershare_sdk::{Device, DiscoveryDelegate};
use intershare_windows::discovery::Discovery;

#[derive(Debug)]
struct DiscoveryTestDelegate {}

impl DiscoveryDelegate for DiscoveryTestDelegate {
    fn device_added(&self, value: Device) {
        println!("Device added: {:?}", value);
    }

    fn device_removed(&self, device_id: String) {
        todo!()
    }
}

#[test]
pub fn test_discovery() {
    let discovery = Discovery::new(Some(Box::new(DiscoveryTestDelegate {})));
    discovery.unwrap().start();
    loop {
        sleep(Duration::from_secs(10))
    }
}
