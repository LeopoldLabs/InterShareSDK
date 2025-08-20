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

// Constants for optimized scanning
private let MAX_CONCURRENT_CONNECTIONS = 5
private let CONNECTION_COOLDOWN: TimeInterval = 2.0 // Shorter cooldown to retrigger attempts faster
private let SCAN_TIMEOUT: TimeInterval = 30.0 // Longer scan periods for better discovery
private let FALLBACK_DELAY: TimeInterval = 10.0 // Switch to broad scan if nothing found after 10s
private let SKIP_THRESHOLD_BEFORE_FORCE_ATTEMPT = 5 // After N skips, override cooldown once

public class BLEClientManager: NSObject, BleDiscoveryImplementationDelegate, CBCentralManagerDelegate, CBPeripheralDelegate {
    private let delegate: DiscoveryDelegate
    private let internalHandler: InternalDiscovery
    private let centralManager = CBCentralManager()
    private var state: BluetoothState = .unknown
    private var discoveredPeripherals: [CBPeripheral] = []
    private var connectionAttempts: [UUID: Date] = [:]
    private var activeConnections: [UUID: Date] = [:]
    private var scanTimer: Timer?
    private var fallbackTimer: Timer?
    private var fallbackBroadScanEnabled: Bool = false
    private var cooldownSkipCounters: [UUID: Int] = [:]
    private var connectRetryCounters: [UUID: Int] = [:]
    private let maxQuickRetries = 2

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

        print("InterShareSDK [BLE Client]: Starting optimized BLE scanning")
        connectionAttempts.removeAll()
        discoveredPeripherals.removeAll()
        activeConnections.removeAll()
        fallbackBroadScanEnabled = false
        cooldownSkipCounters.removeAll()
        connectRetryCounters.removeAll()

        // Start scanning with optimized parameters
        centralManager.scanForPeripherals(withServices: [ServiceUUID], options: [
            CBCentralManagerScanOptionAllowDuplicatesKey: true
        ])
        
        // Set up scan timeout for continuous scanning
        scanTimer = Timer.scheduledTimer(withTimeInterval: SCAN_TIMEOUT, repeats: true) { [weak self] _ in
            self?.restartScanning()
        }

        // Fallback: if no devices found after FALLBACK_DELAY, restart with broad scan
        fallbackTimer?.invalidate()
        fallbackTimer = Timer.scheduledTimer(withTimeInterval: FALLBACK_DELAY, repeats: false) { [weak self] _ in
            guard let self = self else { return }
            if self.discoveredPeripherals.isEmpty {
                print("InterShareSDK [BLE Client]: No devices found after \(Int(FALLBACK_DELAY))s, switching to broad scan")
                self.centralManager.stopScan()
                self.fallbackBroadScanEnabled = true
                self.centralManager.scanForPeripherals(withServices: nil, options: [
                    CBCentralManagerScanOptionAllowDuplicatesKey: true
                ])
            }
        }
    }
    
    private func restartScanning() {
        if centralManager.isScanning {
            print("InterShareSDK [BLE Client]: Restarting scan cycle for continuous discovery")
            centralManager.stopScan()
            if fallbackBroadScanEnabled {
                centralManager.scanForPeripherals(withServices: nil, options: [
                    CBCentralManagerScanOptionAllowDuplicatesKey: true
                ])
            } else {
                centralManager.scanForPeripherals(withServices: [ServiceUUID], options: [
                    CBCentralManagerScanOptionAllowDuplicatesKey: true
                ])
            }
        }
    }

    public func stopScanning() {
        print("InterShareSDK [BLE Client]: Stopping BLE scanning")
        scanTimer?.invalidate()
        scanTimer = nil
        fallbackTimer?.invalidate()
        fallbackTimer = nil
        fallbackBroadScanEnabled = false
        centralManager.stopScan()
        centralManager.delegate = nil
    }

    public func centralManager(_ central: CBCentralManager, didDiscover peripheral: CBPeripheral, advertisementData: [String : Any], rssi RSSI: NSNumber) {
        let currentTime = Date()
        
        // If broad scanning is enabled, filter results by ServiceUUID from advertisement data
        if fallbackBroadScanEnabled {
            if let uuids = advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID] {
                if uuids.contains(ServiceUUID) == false {
                    return
                }
            } else {
                return
            }
        }

        // Check if we've attempted to connect to this device recently
        if let lastAttempt = connectionAttempts[peripheral.identifier] {
            if currentTime.timeIntervalSince(lastAttempt) < CONNECTION_COOLDOWN {
                // Increment skip counter and possibly force a retry
                let skips = (cooldownSkipCounters[peripheral.identifier] ?? 0) + 1
                cooldownSkipCounters[peripheral.identifier] = skips
                if skips < SKIP_THRESHOLD_BEFORE_FORCE_ATTEMPT {
                    print("InterShareSDK [BLE Client]: Skipping connection to \(peripheral.name ?? "Unknown") (cooldown, skip #\(skips))")
                    return
                } else {
                    print("InterShareSDK [BLE Client]: Overriding cooldown after \(skips) skips for \(peripheral.name ?? "Unknown")")
                    cooldownSkipCounters[peripheral.identifier] = 0
                }
            }
        }
        
        // Check concurrent connection limit
        if activeConnections.count >= MAX_CONCURRENT_CONNECTIONS {
            print("InterShareSDK [BLE Client]: Too many concurrent connections (\(MAX_CONCURRENT_CONNECTIONS)), skipping \(peripheral.name ?? "Unknown")")
            return
        }
        
        connectionAttempts[peripheral.identifier] = currentTime
        activeConnections[peripheral.identifier] = currentTime
        cooldownSkipCounters[peripheral.identifier] = 0
        connectRetryCounters[peripheral.identifier] = 0
        peripheral.delegate = self
        discoveredPeripherals.append(peripheral)
        
        print("InterShareSDK [BLE Client]: Found device: \(peripheral.name ?? "Unknown") (\(peripheral.identifier))")
        // Pause scanning during connection to improve reliability on older devices
        if centralManager.isScanning { centralManager.stopScan() }
        central.connect(peripheral)
    }

    public func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        print("InterShareSDK [BLE Client]: Connected to \(peripheral.name ?? "Unknown")")
        cooldownSkipCounters[peripheral.identifier] = 0
        peripheral.discoverServices([ServiceUUID])
    }
    
    public func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral, error: Error?) {
        print("InterShareSDK [BLE Client]: Failed to connect to \(peripheral.name ?? "Unknown"): \(error?.localizedDescription ?? "Unknown error")")
        // Quick retry if encryption timeout is observed, limited attempts
        let message = error?.localizedDescription ?? ""
        let isEncryptionTimeout = message.localizedCaseInsensitiveContains("encrypt") || message.localizedCaseInsensitiveContains("timed out")
        let retryCount = (connectRetryCounters[peripheral.identifier] ?? 0)
        if isEncryptionTimeout && retryCount < maxQuickRetries {
            connectRetryCounters[peripheral.identifier] = retryCount + 1
            print("InterShareSDK [BLE Client]: Retrying connection (\(retryCount + 1)/\(maxQuickRetries)) for \(peripheral.name ?? "Unknown") after encryption timeout")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in
                guard let self = self else { return }
                self.centralManager.connect(peripheral)
            }
            return
        }

        discoveredPeripherals.removeAll { $0 == peripheral }
        activeConnections.removeValue(forKey: peripheral.identifier)
        cooldownSkipCounters[peripheral.identifier] = 0
        connectRetryCounters[peripheral.identifier] = 0
        // Resume scanning after connection attempt completes
        resumeScanningIfNeeded()
    }
    
    public func centralManager(_ central: CBCentralManager, didDisconnectPeripheral peripheral: CBPeripheral, error: Error?) {
        print("InterShareSDK [BLE Client]: Disconnected from \(peripheral.name ?? "Unknown")")
        discoveredPeripherals.removeAll { $0 == peripheral }
        activeConnections.removeValue(forKey: peripheral.identifier)
        cooldownSkipCounters[peripheral.identifier] = 0
        connectRetryCounters[peripheral.identifier] = 0
        // Resume scanning after disconnection
        resumeScanningIfNeeded()
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

    private func resumeScanningIfNeeded() {
        if centralManager.isScanning == false {
            if fallbackBroadScanEnabled {
                centralManager.scanForPeripherals(withServices: nil, options: [
                    CBCentralManagerScanOptionAllowDuplicatesKey: true
                ])
            } else {
                centralManager.scanForPeripherals(withServices: [ServiceUUID], options: [
                    CBCentralManagerScanOptionAllowDuplicatesKey: true
                ])
            }
        }
    }
}
