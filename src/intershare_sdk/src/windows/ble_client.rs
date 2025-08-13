use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use log::{error, info, warn};
use windows::{
    core::{Result, GUID},
    Devices::Bluetooth::{
        Advertisement::{
            BluetoothLEAdvertisementFilter, BluetoothLEAdvertisementReceivedEventArgs,
            BluetoothLEAdvertisementWatcher, BluetoothLEAdvertisementWatcherStatus,
            BluetoothLEScanningMode,
        },
        BluetoothLEDevice,
        GenericAttributeProfile::GattCommunicationStatus,
    },
    Foundation::TypedEventHandler,
    Storage::Streams::DataReader,
};
use tokio::runtime::Handle;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use crate::{BLE_DISCOVERY_CHARACTERISTIC_UUID, BLE_SERVICE_UUID};
use crate::discovery::InternalDiscovery;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// Constants for optimized scanning
const MAX_CONCURRENT_CONNECTIONS: usize = 5;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(8);
const DEVICE_DEDUPLICATION_WINDOW: Duration = Duration::from_secs(3);
const SCAN_INTERVAL: Duration = Duration::from_secs(12);
const PAUSE_INTERVAL: Duration = Duration::from_secs(1);
const CONNECTION_RETRY_DELAY: Duration = Duration::from_millis(200);

impl InternalDiscovery {
    pub(crate) fn windows_start_scanning(self: Arc<Self>) {
        let scanning = self.scanning.clone();
        let self_copy = self.clone();

        scanning.store(true, Ordering::Relaxed);

        std::thread::spawn(move || {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).unwrap();
            }

            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();

            let handle = rt.handle().clone();

            rt.block_on(async {
                if let Err(e) = Self::scan_and_connect(self_copy, scanning, handle).await {
                    error!("Error during scanning: {:?}", e);
                }
            });
        });
    }

    pub(crate) fn windows_stop_scanning(&self) {
        self.scanning.store(false, Ordering::Relaxed);
    }
}

impl InternalDiscovery {
    async fn scan_and_connect(
        internal_discovery: Arc<Self>,
        scanning: Arc<AtomicBool>,
        runtime_handle: Handle
    ) -> Result<()> {
        let mut watcher = BluetoothLEAdvertisementWatcher::new()?;

        // Set up the filter for the service UUID
        let filter = BluetoothLEAdvertisementFilter::new()?;
        filter.Advertisement()?.ServiceUuids()?.Append(GUID::from(BLE_SERVICE_UUID))?;
        watcher.SetAdvertisementFilter(&filter)?;

        // Use active scanning for maximum discovery efficiency
        watcher.SetScanningMode(BluetoothLEScanningMode::Active)?;

        let discovered_devices = Arc::new(Mutex::new(HashMap::<u64, Instant>::new()));
        let active_connections = Arc::new(Mutex::new(HashMap::<u64, Instant>::new()));
        let discovered_devices_clone = discovered_devices.clone();
        let active_connections_clone = active_connections.clone();
        let internal_discovery_clone = internal_discovery.clone();

        let handler = TypedEventHandler::new(
            move |_: &Option<BluetoothLEAdvertisementWatcher>,
                  args: &Option<BluetoothLEAdvertisementReceivedEventArgs>| {
                let args = args.as_ref().unwrap();
                let ble_address = args.BluetoothAddress()?;
                let discovered_devices = discovered_devices_clone.clone();
                let active_connections = active_connections_clone.clone();
                let internal_discovery = internal_discovery_clone.clone();

                let advertisement = args.Advertisement()?;
                let local_name = advertisement.LocalName()?.to_string();

                // Check if we've seen this device recently (within 3 seconds for faster discovery)
                let mut devices = discovered_devices.lock().unwrap();
                let now = Instant::now();
                if let Some(last_seen) = devices.get(&ble_address) {
                    if now.duration_since(*last_seen) < DEVICE_DEDUPLICATION_WINDOW {
                        return Ok(());
                    }
                }
                devices.insert(ble_address, now);

                // Check concurrent connection limit
                let mut connections = active_connections.lock().unwrap();
                if connections.len() >= MAX_CONCURRENT_CONNECTIONS {
                    warn!("Too many concurrent connections ({}), skipping {}", MAX_CONCURRENT_CONNECTIONS, local_name);
                    return Ok(());
                }
                connections.insert(ble_address, now);

                info!("Discovered device: {} (address: {})", local_name, ble_address);

                runtime_handle.spawn(async move {
                    if let Err(e) = Self::connect_and_read_characteristic(ble_address, internal_discovery, local_name).await {
                        warn!("Error connecting to device {}: {:?}", local_name, e);
                    }
                    
                    // Remove from active connections
                    let mut connections = active_connections.lock().unwrap();
                    connections.remove(&ble_address);
                });

                Ok(())
            },
        );

        watcher.Received(&handler)?;
        watcher.Start()?;

        info!("Started optimized BLE advertisement watcher");

        // Optimized scanning intervals for maximum efficiency
        let mut is_scanning = true;

        while scanning.load(Ordering::Relaxed) {
            if is_scanning {
                tokio::time::sleep(SCAN_INTERVAL).await;
                is_scanning = false;
                
                // Brief pause to allow other operations
                if watcher.Status()? == BluetoothLEAdvertisementWatcherStatus::Started {
                    watcher.Stop()?;
                    info!("Paused BLE scanning for brief interval");
                }
            } else {
                tokio::time::sleep(PAUSE_INTERVAL).await;
                is_scanning = true;
                
                // Resume scanning
                if watcher.Status()? != BluetoothLEAdvertisementWatcherStatus::Started {
                    watcher.Start()?;
                    info!("Resumed BLE scanning");
                }
            }
        }

        if watcher.Status()? == BluetoothLEAdvertisementWatcherStatus::Started {
            watcher.Stop()?;
            info!("Stopped BLE advertisement watcher");
        }

        Ok(())
    }

    async fn connect_and_read_characteristic(
        ble_address: u64,
        internal_discovery: Arc<Self>,
        device_name: String
    ) -> Result<()> {
        // Optimized connection with shorter timeout and faster retry
        let max_retries = 2;
        let mut retry_count = 0;

        while retry_count < max_retries {
            match tokio::time::timeout(CONNECTION_TIMEOUT, Self::attempt_connection(ble_address, &internal_discovery, &device_name)).await {
                Ok(Ok(_)) => {
                    info!("Successfully connected to device: {}", device_name);
                    return Ok(());
                }
                Ok(Err(e)) => {
                    retry_count += 1;
                    warn!("Connection attempt {} failed for device {}: {:?}", retry_count, device_name, e);
                    
                    if retry_count < max_retries {
                        // Shorter backoff for faster retry
                        tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
                    }
                }
                Err(_) => {
                    retry_count += 1;
                    warn!("Connection timeout for device {}", device_name);
                    
                    if retry_count < max_retries {
                        tokio::time::sleep(CONNECTION_RETRY_DELAY).await;
                    }
                }
            }
        }

        error!("Failed to connect to device {} after {} attempts", device_name, max_retries);
        Ok(())
    }

    async fn attempt_connection(
        ble_address: u64,
        internal_discovery: &Arc<Self>,
        device_name: &str
    ) -> Result<()> {
        // Connect to the device with optimized timeout
        let device = BluetoothLEDevice::FromBluetoothAddressAsync(ble_address)?.get()?;
        let device_id = device.DeviceId()?.to_string();
        info!("Attempting connection to device: \"{:?}\" ID: {:?}", device_name, device_id);

        // Get the GATT services with timeout handling
        let services_result = device.GetGattServicesForUuidAsync(GUID::from(BLE_SERVICE_UUID))?.get()?;
        if services_result.Status()? != GattCommunicationStatus::Success {
            error!("[{}, {:?}] Failed to get GATT services", device_name, device_id);
            return Err(windows::core::Error::new(windows::core::E_FAIL, "Failed to get GATT services"));
        }
        let services = services_result.Services()?;

        if services.Size()? == 0 {
            error!("[{}, {:?}] No services found", device_name, device_id);
            return Err(windows::core::Error::new(windows::core::E_FAIL, "No services found"));
        }

        let service = services.GetAt(0)?;

        // Get the characteristics
        let characteristics_result = service.GetCharacteristicsForUuidAsync(GUID::from(BLE_DISCOVERY_CHARACTERISTIC_UUID))?.get()?;
        if characteristics_result.Status()? != GattCommunicationStatus::Success {
            error!("[{}, {:?}] Failed to get characteristics", device_name, device_id);
            return Err(windows::core::Error::new(windows::core::E_FAIL, "Failed to get characteristics"));
        }
        let characteristics = characteristics_result.Characteristics()?;

        if characteristics.Size()? == 0 {
            error!("[{}, {:?}] No characteristics found", device_name, device_id);
            return Err(windows::core::Error::new(windows::core::E_FAIL, "No characteristics found"));
        }

        let characteristic = characteristics.GetAt(0)?;

        // Read the characteristic value
        let read_result = characteristic.ReadValueAsync()?.get()?;
        if read_result.Status()? != GattCommunicationStatus::Success {
            error!("[{}, {:?}] Failed to read characteristic", device_name, device_id);
            return Err(windows::core::Error::new(windows::core::E_FAIL, "Failed to read characteristic"));
        }
        let value = read_result.Value()?;
        let reader = DataReader::FromBuffer(&value)?;
        let length = reader.UnconsumedBufferLength()? as usize;
        let mut buffer = vec![0u8; length];
        reader.ReadBytes(&mut buffer)?;

        internal_discovery.parse_discovery_message(buffer, Some(device_id));

        Ok(())
    }
}
