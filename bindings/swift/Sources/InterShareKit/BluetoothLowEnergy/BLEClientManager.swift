//
//  BleClient.swift
//
//
//  Created by Julian Baumann on 06.01.24.
//

import Foundation
import CoreBluetooth

public enum OpenL2CAPErrors: Error {
    case PeripheralNotFound
}

public class BLEClientManager: NSObject, BleDiscoveryImplementationDelegate, CBCentralManagerDelegate, CBPeripheralDelegate {
    private let delegate: DiscoveryDelegate
    private let internalHandler: InternalDiscovery
    private let centralManager = CBCentralManager()
    private var state: BluetoothState = .unknown
    private var discoveredPeripherals: [CBPeripheral] = []
    private var connectionAttempts: [UUID: Date] = [:]
    private let connectionCooldown: TimeInterval = 10.0 // 10 seconds

    init(delegate: DiscoveryDelegate, internalHandler: InternalDiscovery) {
        self.delegate = delegate
        self.internalHandler = internalHandler

        super.init()
        centralManager.delegate = self
    }

    public func ensureValidState() throws {
        if state != .poweredOn {
            throw InvalidStateError()
        }
    }

    public func centralManagerDidUpdateState(_ central: CBCentralManager) {
        state = BluetoothState(from: central.state)
        delegate.discoveryDidUpdateState(state: state)
        
        if state == .poweredOn {
            print("InterShareSDK [BLE Client]: Bluetooth is powered on")
        } else {
            print("InterShareSDK [BLE Client]: Bluetooth state changed to: \(state)")
        }
    }

    public func startScanning() {
        if centralManager.isScanning {
            print("InterShareSDK [BLE Client]: Already scanning, ignoring start request")
            return
        }

        print("InterShareSDK [BLE Client]: Starting BLE scanning")
        connectionAttempts.removeAll()
        discoveredPeripherals.removeAll()

        centralManager.scanForPeripherals(withServices: [ServiceUUID], options: [
            CBCentralManagerScanOptionAllowDuplicatesKey: true
        ])
    }

    public func stopScanning() {
        print("InterShareSDK [BLE Client]: Stopping BLE scanning")
        centralManager.stopScan()
        centralManager.delegate = nil
    }

    public func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String : Any], rssi RSSI: NSNumber) {
        let currentTime = Date()
        
        // Check if we've attempted to connect to this device recently
        if let lastAttempt = connectionAttempts[peripheral.identifier] {
            if currentTime.timeIntervalSince(lastAttempt) < connectionCooldown {
                print("InterShareSDK [BLE Client]: Skipping connection to \(peripheral.name ?? "Unknown") (cooldown)")
                return
            }
        }
        
        connectionAttempts[peripheral.identifier] = currentTime
        peripheral.delegate = self
        discoveredPeripherals.append(peripheral)
        
        print("InterShareSDK [BLE Client]: Found device: \(peripheral.name ?? "Unknown") (\(peripheral.identifier))")
        central.connect(peripheral)
    }

    public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        print("InterShareSDK [BLE Client]: Connected to \(peripheral.name ?? "Unknown")")
        peripheral.discoverServices([ServiceUUID])
    }
    
    public func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        print("InterShareSDK [BLE Client]: Failed to connect to \(peripheral.name ?? "Unknown"): \(error?.localizedDescription ?? "Unknown error")")
        discoveredPeripherals.removeAll { $0 == peripheral }
    }
    
    public func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        print("InterShareSDK [BLE Client]: Disconnected from \(peripheral.name ?? "Unknown")")
        discoveredPeripherals.removeAll { $0 == peripheral }
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        if let error = error {
            print("InterShareSDK [BLE Client]: Service discovery failed for \(peripheral.name ?? "Unknown"): \(error.localizedDescription)")
            centralManager.cancelPeripheralConnection(peripheral)
            return
        }

        let service = peripheral.services?.first(where: { $0.uuid == ServiceUUID })

        guard let service = service else {
            print("InterShareSDK [BLE Client]: Service not found for \(peripheral.name ?? "Unknown")")
            centralManager.cancelPeripheralConnection(peripheral)
            return
        }

        print("InterShareSDK [BLE Client]: Discovering characteristics for \(peripheral.name ?? "Unknown")")
        peripheral.discoverCharacteristics([DiscoveryCharacteristicUUID], for: service)
    }

    public func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService, error: Error?) {
        if let error = error {
            print("InterShareSDK [BLE Client]: Characteristic discovery failed for \(peripheral.name ?? "Unknown"): \(error.localizedDescription)")
            centralManager.cancelPeripheralConnection(peripheral)
            return
        }

        let characteristic = service.characteristics?.first(where: { $0.uuid == DiscoveryCharacteristicUUID })

        if let characteristic = characteristic {
            print("InterShareSDK [BLE Client]: Reading characteristic for \(peripheral.name ?? "Unknown")")
            peripheral.readValue(for: characteristic)
        } else {
            print("InterShareSDK [BLE Client]: Discovery characteristic not found for \(peripheral.name ?? "Unknown")")
            centralManager.cancelPeripheralConnection(peripheral)
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didModifyServices invalidatedServices: [CBService]) {
        print("InterShareSDK [BLE Client]: Services modified for \(peripheral.name ?? "Unknown")")
        peripheral.discoverServices([ServiceUUID])
        for service in invalidatedServices {
            if discoveredPeripherals.contains(where: { $0 == service.peripheral } ) {
                discoveredPeripherals.removeAll(where: { $0 == service.peripheral })
            }
        }
    }

    public func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic, error: Error?) {
        if let error = error {
            print("InterShareSDK [BLE Client]: Characteristic read failed for \(peripheral.name ?? "Unknown"): \(error.localizedDescription)")
            centralManager.cancelPeripheralConnection(peripheral)
            return
        }

        let data = characteristic.value

        if let data = data {
            print("InterShareSDK [BLE Client]: Successfully read characteristic for \(peripheral.name ?? "Unknown")")
            internalHandler.parseDiscoveryMessage(data: data, bleUuid: peripheral.identifier.uuidString)
            centralManager.cancelPeripheralConnection(peripheral)
        } else {
            print("InterShareSDK [BLE Client]: No data received from characteristic for \(peripheral.name ?? "Unknown")")
            centralManager.cancelPeripheralConnection(peripheral)
        }
    }
}
