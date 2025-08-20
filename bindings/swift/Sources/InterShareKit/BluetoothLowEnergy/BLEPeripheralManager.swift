//
//  File.swift
//  
//
//  Created by Julian Baumann on 05.01.24.
//

import Foundation
import CoreBluetooth

struct InvalidStateError: Error {}

// Constants for optimized advertising
private let ADVERTISING_RETRY_DELAY: TimeInterval = 1.0
private let MAX_ADVERTISING_RETRIES = 3

class BLEPeripheralManager: NSObject, BleServerImplementationDelegate, CBPeripheralManagerDelegate {
    private let peripheralManager: CBPeripheralManager
    private let internalHandler: InternalNearbyServer
    private let nearbyServerDelegate: NearbyServerDelegate
    private var streams: [L2CapStream] = []
    private var advertisingRetryCount = 0

    private var isPoweredOn = false
    public var state: BluetoothState

    init(handler: InternalNearbyServer, delegate: NearbyServerDelegate) {
        nearbyServerDelegate = delegate
        internalHandler = handler
        peripheralManager = CBPeripheralManager()
        state = BluetoothState(from: peripheralManager.state)
        
        super.init()
        peripheralManager.delegate = self
    }
    
    func peripheralManagerDidUpdateState(_ peripheral: CBPeripheralManager) {
        state = BluetoothState(from: peripheral.state)
        nearbyServerDelegate.nearbyServerDidUpdateState(state: state)
        
        if state == .poweredOn {
            print("InterShareSDK [BLE Peripheral]: Bluetooth is powered on, ready for advertising")
        } else {
            print("InterShareSDK [BLE Peripheral]: Bluetooth state changed to: \(state)")
        }
    }
    
    public func ensureValidState() throws {
        if state != .poweredOn {
            throw InvalidStateError()
        }
    }
    
    func startL2CapServer() {
        peripheralManager.publishL2CAPChannel(withEncryption: false)
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didPublishL2CAPChannel PSM: CBL2CAPPSM, error: Error?) {
        print("L2CAP Channel PSM: \(PSM)")
        internalHandler.setBluetoothLeDetails(bleInfo: BluetoothLeConnectionInfo(uuid: "", psm: UInt32(PSM)))
        addService()
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didOpen channel: CBL2CAPChannel?, error: Error?) {
        print("L2CAP Channel was opened")
        
        guard let channel else {
            return
        }
        
        let l2capStream = L2CapStream(channel: channel)
        streams.append(l2capStream)

        Task {
            internalHandler.handleIncomingConnection(nativeStreamHandle: l2capStream)
        }
    }
    
    func addService() {
        let service = CBMutableService(type: ServiceUUID, primary: true)
        let discoveryCharacteristic = CBMutableCharacteristic(
            type: DiscoveryCharacteristicUUID,
            properties: [.read],
            value: nil,
            permissions: CBAttributePermissions.readable
        )

        let writeCharacteristic = CBMutableCharacteristic(
            type: WriteCharacteristicUUID,
            properties: [.write],
            value: nil,
            permissions: CBAttributePermissions.writeable
        )

        service.characteristics = [discoveryCharacteristic, writeCharacteristic]
        
        peripheralManager.add(service)
        // Do NOT start advertising here; wait for didAdd callback so the service is fully registered
    }
    
    private func startOptimizedAdvertising() {
        // Optimized advertising data for maximum discoverability (no local name to keep legacy ADV)
        let advertisingData: [String: Any] = [
            CBAdvertisementDataServiceUUIDsKey: [ServiceUUID],
            CBAdvertisementDataIsConnectable: true
        ]
        
        print("InterShareSDK [BLE Peripheral]: Starting optimized advertising")
        peripheralManager.startAdvertising(advertisingData)
    }
    
    private func retryAdvertising() {
        if advertisingRetryCount < MAX_ADVERTISING_RETRIES {
            advertisingRetryCount += 1
            print("InterShareSDK [BLE Peripheral]: Retrying advertising attempt \(advertisingRetryCount)")
            
            DispatchQueue.global().asyncAfter(deadline: .now() + ADVERTISING_RETRY_DELAY * Double(advertisingRetryCount)) { [weak self] in
                self?.startOptimizedAdvertising()
            }
        } else {
            print("InterShareSDK [BLE Peripheral]: Failed to start advertising after \(MAX_ADVERTISING_RETRIES) attempts")
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didAdd service: CBService, error: Error?) {
        if let error = error {
            print("InterShareSDK [BLE Peripheral]: Failed to add service: \(error.localizedDescription)")
        } else {
            print("InterShareSDK [BLE Peripheral]: Service added successfully")
            // Start advertising only after the service has been added
            startOptimizedAdvertising()
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveRead request: CBATTRequest) {
        Task {
            request.value = await internalHandler.getAdvertisementData()
            peripheral.respond(to: request, withResult: CBATTError.success)
        }
    }
    
    func peripheralManager(_ peripheral: CBPeripheralManager, didReceiveWrite requests: [CBATTRequest]) {
        // Handle write requests if needed
    }
    
    func peripheralManagerDidStartAdvertising(_ peripheral: CBPeripheralManager, error: Error?) {
        if let error = error {
            print("InterShareSDK [BLE Peripheral]: Advertising failed: \(error.localizedDescription)")
            retryAdvertising()
        } else {
            print("InterShareSDK [BLE Peripheral]: Advertising started successfully")
            advertisingRetryCount = 0 // Reset retry count on success
        }
    }
    
    func startServer() {
        print("InterShareSDK [BLE Peripheral]: Starting optimized server")
        startL2CapServer()
    }
    
    func stopServer() {
        print("InterShareSDK [BLE Peripheral]: Stopping server")
        peripheralManager.stopAdvertising()
        peripheralManager.removeAllServices()
    }
}
