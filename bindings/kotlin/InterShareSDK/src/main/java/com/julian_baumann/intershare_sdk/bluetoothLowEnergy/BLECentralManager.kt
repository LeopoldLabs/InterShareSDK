package com.julian_baumann.intershare_sdk.bluetoothLowEnergy

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.*
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.ParcelUuid
import android.util.Log
import androidx.core.app.ActivityCompat
import com.julian_baumann.intershare_sdk.BleDiscoveryImplementationDelegate
import com.julian_baumann.intershare_sdk.InternalDiscovery
import kotlinx.coroutines.*
import java.util.*
import java.util.concurrent.ConcurrentHashMap


@SuppressLint("MissingPermission")
class BluetoothGattCallbackImplementation(
    private val internal: InternalDiscovery,
    private var currentlyConnectedDevices: MutableList<BluetoothDevice>,
    private var discoveredPeripherals: MutableList<BluetoothDevice>) : BluetoothGattCallback() {
    override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
        when (newState) {
            BluetoothProfile.STATE_CONNECTED -> {
                Log.d("InterShareSDK [BLE Central]", "Connected to ${gatt.device.name}")
                gatt.requestMtu(150)
            }
            BluetoothProfile.STATE_DISCONNECTED -> {
                Log.d("InterShareSDK [BLE Central]", "Disconnected from ${gatt.device.name}")
                gatt.close()
                currentlyConnectedDevices.remove(gatt.device)
            }
            else -> {
                Log.d("InterShareSDK [BLE Central]", "Connection state changed to: $newState for ${gatt.device.name}")
                currentlyConnectedDevices.remove(gatt.device)
            }
        }
    }

    override fun onMtuChanged(gatt: BluetoothGatt?, mtu: Int, status: Int) {
        if (status == BluetoothGatt.GATT_SUCCESS) {
            Log.d("InterShareSDK [BLE Central]", "MTU changed to $mtu, discovering services")
            gatt?.discoverServices()
        } else {
            Log.w("InterShareSDK [BLE Central]", "MTU change failed with status: $status")
        }
    }

    override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
        if (status == BluetoothGatt.GATT_SUCCESS) {
            Log.d("InterShareSDK [BLE Central]", "Services discovered for ${gatt.device.name}")
            getDeviceInfo(gatt)
        } else {
            Log.w("InterShareSDK [BLE Central]", "Service discovery failed with status: $status")
        }
    }

    @SuppressLint("MissingPermission")
    private fun getDeviceInfo(gatt: BluetoothGatt) {
        val service = gatt.getService(discoveryServiceUUID)

        service?.let {
            val characteristic = it.getCharacteristic(discoveryCharacteristicUUID)
            if (characteristic != null) {
                gatt.readCharacteristic(characteristic)
            } else {
                Log.w("InterShareSDK [BLE Central]", "Discovery characteristic not found")
                gatt.disconnect()
            }
        } ?: run {
            Log.w("InterShareSDK [BLE Central]", "Discovery service not found")
            gatt.disconnect()
        }
    }

    override fun onCharacteristicChanged(
        gatt: BluetoothGatt,
        characteristic: BluetoothGattCharacteristic,
        value: ByteArray
    ) {
        super.onCharacteristicChanged(gatt, characteristic, value)
        internal.parseDiscoveryMessage(value, gatt.device.address)
    }

    // Still needed for older Android versions (< 13)
    @Deprecated("Deprecated")
    override fun onCharacteristicRead(
        gatt: BluetoothGatt?,
        characteristic: BluetoothGattCharacteristic?,
        status: Int
    ) {
        super.onCharacteristicRead(gatt, characteristic, status)

        if (gatt != null && characteristic != null && characteristic.value != null) {
            handleCharacteristicData(characteristic.value, status, gatt)
        }
    }

    override fun onCharacteristicRead(
        gatt: BluetoothGatt,
        characteristic: BluetoothGattCharacteristic,
        value: ByteArray,
        status: Int
    ) {
        super.onCharacteristicRead(gatt, characteristic, value, status)
        handleCharacteristicData(value, status, gatt)
    }

    private fun handleCharacteristicData(data: ByteArray, status: Int, gatt: BluetoothGatt) {
        if (status == BluetoothGatt.GATT_SUCCESS) {
            Log.d("InterShareSDK [BLE Central]", "GATT READ was successful for ${gatt.device.name}")

            internal.parseDiscoveryMessage(data, gatt.device.address)

            if (!discoveredPeripherals.contains(gatt.device)) {
                discoveredPeripherals.add(gatt.device)
            }

            // Disconnect after successful read
            gatt.disconnect()
        } else {
            Log.w("InterShareSDK [BLE Central]", "GATT READ failed with status: $status for ${gatt.device.name}")
            gatt.disconnect()
        }
    }

    private fun subscribeToCharacteristic(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic) {
        if (characteristic.properties and BluetoothGattCharacteristic.PROPERTY_NOTIFY != 0) {
            characteristic.writeType = BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT

            gatt.setCharacteristicNotification(characteristic, true)
            val uuid = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
            val descriptor = characteristic.getDescriptor(uuid)
            descriptor.setValue(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
            gatt.writeDescriptor(descriptor)
        }
    }
}

class BLECentralManager(private val context: Context, private val internal: InternalDiscovery) : BleDiscoveryImplementationDelegate {
    private val bluetoothAdapter: BluetoothManager by lazy {
        context.getSystemService(Context.BLUETOOTH_SERVICE) as BluetoothManager
    }

    private var scanJob: Job? = null
    private val scanIntervalMillis = 8000L
    private val pauseBetweenScans = 2000L
    private var isScanning = false

    companion object {
        var discoveredPeripherals = mutableListOf<BluetoothDevice>()
        var currentlyConnectedDevices = mutableListOf<BluetoothDevice>()
        private val connectionAttempts = ConcurrentHashMap<String, Long>()
        private const val CONNECTION_COOLDOWN_MS = 10000L // 10 seconds
    }

    override fun startScanning() {
        if (isScanning) {
            Log.d("InterShareSDK [BLE Central]", "Already scanning, ignoring start request")
            return
        }

        isScanning = true
        discoveredPeripherals.clear()
        currentlyConnectedDevices.clear()
        connectionAttempts.clear()

        if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_SCAN) != PackageManager.PERMISSION_GRANTED) {
            throw BlePermissionNotGrantedException()
        }

        val scanFilter: List<ScanFilter> = listOf(
            ScanFilter.Builder()
                .setServiceUuid(ParcelUuid(discoveryServiceUUID))
                .build()
        )

        val settings = ScanSettings.Builder()
            .setLegacy(false)
            .setPhy(ScanSettings.PHY_LE_ALL_SUPPORTED)
            .setNumOfMatches(ScanSettings.MATCH_NUM_ONE_ADVERTISEMENT)
            .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
            .setMatchMode(ScanSettings.MATCH_MODE_AGGRESSIVE)
            .setReportDelay(0L)
            .build()

        scanJob = CoroutineScope(Dispatchers.IO).launch {
            while (isActive) {
                try {
                    Log.d("InterShareSDK [BLE Central]", "Starting scan cycle")
                    bluetoothAdapter.adapter.bluetoothLeScanner.startScan(scanFilter, settings, leScanCallback)
                    delay(scanIntervalMillis)
                    bluetoothAdapter.adapter.bluetoothLeScanner.stopScan(leScanCallback)
                    Log.d("InterShareSDK [BLE Central]", "Pausing scan cycle")
                    delay(pauseBetweenScans)
                } catch (e: Exception) {
                    Log.e("InterShareSDK [BLE Central]", "Error during scanning: ${e.message}")
                    delay(1000) // Brief pause on error
                }
            }
        }
    }

    override fun stopScanning() {
        if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_SCAN) != PackageManager.PERMISSION_GRANTED) {
            throw BlePermissionNotGrantedException()
        }

        Log.d("InterShareSDK [BLE Central]", "Stopping BLE scanning")
        scanJob?.cancel()
        bluetoothAdapter.adapter.bluetoothLeScanner.stopScan(leScanCallback)
        isScanning = false
    }

    @SuppressLint("MissingPermission")
    private val leScanCallback: ScanCallback = object : ScanCallback() {
        fun addDevice(device: BluetoothDevice) {
            val deviceAddress = device.address
            val currentTime = System.currentTimeMillis()
            
            // Check if we've attempted to connect to this device recently
            val lastAttempt = connectionAttempts[deviceAddress]
            if (lastAttempt != null && (currentTime - lastAttempt) < CONNECTION_COOLDOWN_MS) {
                Log.d("InterShareSDK [BLE Central]", "Skipping connection to ${device.name} (cooldown)")
                return
            }

            if (!currentlyConnectedDevices.contains(device)) {
                currentlyConnectedDevices.add(device)
                connectionAttempts[deviceAddress] = currentTime
                
                Log.d("InterShareSDK [BLE Central]", "Found device: ${device.name} (${device.address})")

                device.connectGatt(
                    context,
                    false,
                    BluetoothGattCallbackImplementation(internal, currentlyConnectedDevices, discoveredPeripherals),
                    BluetoothDevice.TRANSPORT_LE,
                    BluetoothDevice.PHY_LE_2M_MASK
                )
            }
        }

        override fun onScanResult(callbackType: Int, result: ScanResult) {
            addDevice(result.device)
        }

        override fun onBatchScanResults(results: List<ScanResult>) {
            results.forEach { result ->
                addDevice(result.device)
            }
        }

        override fun onScanFailed(errorCode: Int) {
            Log.e("InterShareSDK [BLE Central]", "Scan failed with error code: $errorCode")
        }
    }
}
