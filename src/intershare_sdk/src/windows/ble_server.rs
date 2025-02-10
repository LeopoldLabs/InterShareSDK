use windows::{
    core::{Result as WinResult, GUID},
    Devices::Bluetooth::GenericAttributeProfile::*,
    Foundation::TypedEventHandler,
    Storage::Streams::*,
};
use protocol::discovery::device_discovery_message::Content;
use protocol::discovery::DeviceDiscoveryMessage;
use protocol::prost::Message;
use crate::{BLE_DISCOVERY_CHARACTERISTIC_UUID, BLE_SERVICE_UUID};
use crate::nearby_server::InternalNearbyServer;

impl InternalNearbyServer {
    pub(crate) async fn setup_gatt_server(&self) -> WinResult<GattServiceProvider> {
        let service_uuid = GUID::from(BLE_SERVICE_UUID);

        let service_provider_result: GattServiceProviderResult = GattServiceProvider::CreateAsync(service_uuid)?.get()?;
        let gatt_service_provider = service_provider_result.ServiceProvider()?;

        let characteristic_uuid = GUID::from(BLE_DISCOVERY_CHARACTERISTIC_UUID);

        let characteristic_parameters = GattLocalCharacteristicParameters::new()?;
        characteristic_parameters.SetCharacteristicProperties(
            GattCharacteristicProperties::Read
        )?;

        characteristic_parameters.SetReadProtectionLevel(GattProtectionLevel::Plain)?;

        let characteristic_result: GattLocalCharacteristicResult = gatt_service_provider
            .Service()?
            .CreateCharacteristicAsync(characteristic_uuid, &characteristic_parameters)?
            .get()?;

        let gatt_characteristic = characteristic_result.Characteristic()?;
        let device_connection_info = self.device_connection_info.read().await.clone();

        let read_requested_handler = TypedEventHandler::new(
            move |_sender: &Option<GattLocalCharacteristic>, args: &Option<GattReadRequestedEventArgs>| {
                if let Some(args) = args {
                    let deferral = args.GetDeferral()?;
                    let request: GattReadRequest = args.GetRequestAsync()?.get()?;

                    let value = DeviceDiscoveryMessage {
                        content: Some(
                            Content::DeviceConnectionInfo(
                                device_connection_info.clone()
                            )
                        ),
                    }.encode_length_delimited_to_vec();

                    let writer = DataWriter::new()?;
                    writer.WriteBytes(&value)?;
                    let buffer = writer.DetachBuffer()?;
                    request.RespondWithValue(&buffer)?;
                    deferral.Complete()?;
                }
                Ok(())
            },
        );

        gatt_characteristic.ReadRequested(&read_requested_handler)?;

        return Ok(gatt_service_provider);
    }
    
    
    pub(crate) async fn start_windows_server(&self) {
        let gatt_service_provider = self.setup_gatt_server().await.expect("Failed to start GATT server");

        let mut writable_gatt_service = self.gatt_service_provider
            .write()
            .expect("Failed to unwrap gatt_service_provider");

        let service_provider = writable_gatt_service.insert(gatt_service_provider);

        let adv_parameters = GattServiceProviderAdvertisingParameters::new().expect("Failed to create new GattServiceProviderAdvertisingParameters");
        adv_parameters.SetIsConnectable(true).expect("Failed to set IsConnectable");
        adv_parameters.SetIsDiscoverable(true).expect("Failed to set IsDiscoverable");
        service_provider.StartAdvertisingWithParameters(&adv_parameters).expect("Failed to start Advertising");
    }

    pub(crate) fn stop_windows_server(&self) {
        let gatt_service_provider = self.gatt_service_provider
            .read()
            .expect("Failed to lock GattServiceProvider");

        if let Some(gatt_service_provider) = gatt_service_provider.as_ref() {
            gatt_service_provider.StopAdvertising().expect("Failed to stop advertising");
        }
    }
}
