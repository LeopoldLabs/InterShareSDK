package com.julian_baumann.intershare_sdk.bluetoothLowEnergy

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.*
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.content.Context
import android.content.pm.PackageManager
import android.os.ParcelUuid
import android.util.Log
import androidx.core.app.ActivityCompat
import com.julian_baumann.intershare_sdk.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.util.*

// Constants for optimized advertising
private const val ADVERTISING_RETRY_DELAY_MS = 1000L
private const val MAX_ADVERTISING_RETRIES = 3

class BlePermissionNotGrantedException : Exception()
val discoveryServiceUUID: UUID = UUID.fromString(getBleServiceUuid())
val discoveryCharacteristicUUID: UUID = UUID.fromString(getBleDiscoveryCharacteristicUuid())

internal class BLEPeripheralManager(private val context: Context, private val internalNearbyServer: InternalNearbyServer, private val bluetoothManager: BluetoothManager) : BleServerImplementationDelegate {

    private var bluetoothGattServer: BluetoothGattServer? = null
    private var bluetoothL2CAPServer: BluetoothServerSocket? = null
    private var l2CAPThread: Thread? = null
    private var advertisingRetryCount = 0

    private fun createService(): BluetoothGattService {
        val service = BluetoothGattService(discoveryServiceUUID, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        val characteristic = BluetoothGattCharacteristic(discoveryCharacteristicUUID, BluetoothGattCharacteristic.PROPERTY_READ, BluetoothGattCharacteristic.PERMISSION_READ)

        service.addCharacteristic(characteristic)

        return service
    }

    private val gattServerCallback = object : BluetoothGattServerCallback() {
        override fun onCharacteristicReadRequest(
            device: BluetoothDevice?,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic?
        ) {
            if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_ADVERTISE) != PackageManager.PERMISSION_GRANTED) {
                throw BlePermissionNotGrantedException()
            }

            CoroutineScope(Dispatchers.Main).launch {
                val data = internalNearbyServer.getAdvertisementData()

                bluetoothGattServer?.sendResponse(device,
                    requestId,
                    BluetoothGatt.GATT_SUCCESS,
                    0,
                    data
                )
            }
        }
    }

    private val advertiseCallback = object : AdvertiseCallback() {
        override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
            Log.i("InterShareSDK [BLE Manager]", "LE Advertise Started Successfully")
            advertisingRetryCount = 0 // Reset retry count on success
        }

        override fun onStartFailure(errorCode: Int) {
            Log.w("InterShareSDK [BLE Manager]", "LE Advertise Failed: $errorCode")
            
            // Retry advertising with exponential backoff
            if (advertisingRetryCount < MAX_ADVERTISING_RETRIES) {
                advertisingRetryCount++
                Log.d("InterShareSDK [BLE Manager]", "Retrying advertising attempt $advertisingRetryCount")
                
                CoroutineScope(Dispatchers.IO).launch {
                    kotlinx.coroutines.delay(ADVERTISING_RETRY_DELAY_MS * advertisingRetryCount)
                    startAdvertising()
                }
            } else {
                Log.e("InterShareSDK [BLE Manager]", "Failed to start advertising after $MAX_ADVERTISING_RETRIES attempts")
            }
        }
    }

    private fun startGattServer() {
        if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_ADVERTISE) != PackageManager.PERMISSION_GRANTED) {
            throw BlePermissionNotGrantedException()
        }

        bluetoothL2CAPServer = bluetoothManager.adapter.listenUsingInsecureL2capChannel()

        l2CAPThread = Thread {
            try {
                val psm = bluetoothL2CAPServer!!.psm.toUInt()
                internalNearbyServer.setBluetoothLeDetails(BluetoothLeConnectionInfo("", psm))

                while (true) {
                    val connection = bluetoothL2CAPServer!!.accept()
                    val stream = L2CAPStream(connection)

                    CoroutineScope(Dispatchers.Main).launch {
                        internalNearbyServer.handleIncomingConnection(stream)
                    }
                }
            }
            catch (e: Exception) {
                Log.e("InterShareSDK [BLE Manager]", e.toString())
            }
        }

        l2CAPThread!!.start()

        bluetoothGattServer = bluetoothManager.openGattServer(context, gattServerCallback)
        bluetoothGattServer?.addService(createService())
            ?: Log.w("InterShareSDK [BLE Manager]", "Unable to create GATT server")
    }

    private fun stopGattServer() {
        if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_ADVERTISE) != PackageManager.PERMISSION_GRANTED) {
            throw BlePermissionNotGrantedException()
        }

        bluetoothGattServer?.close()
    }

    @SuppressLint("MissingPermission")
    private fun startAdvertising() {
        val bluetoothLeAdvertiser: BluetoothLeAdvertiser? = bluetoothManager.adapter.bluetoothLeAdvertiser
        bluetoothManager.adapter.setName(internalNearbyServer.getDeviceName())

        bluetoothLeAdvertiser?.let {
            // Optimized advertising settings for maximum discoverability
            val settings = AdvertiseSettings.Builder()
                .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
                .setConnectable(true)
                .setTimeout(0) // No timeout for continuous advertising
                .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_HIGH) // Use high power for better range
                .build()

            // Optimized advertisement data for better discovery
            val data = AdvertiseData.Builder()
                .setIncludeDeviceName(false) // Don't include in main advertisement for faster processing
                .setIncludeTxPowerLevel(true) // Include TX power for better ranging
                .addServiceUuid(ParcelUuid(discoveryServiceUUID))
                .build()

            // Scan response data with device name
            val scanResult = AdvertiseData.Builder()
                .setIncludeDeviceName(false)
                .setIncludeTxPowerLevel(false)
                .build()

            if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_ADVERTISE) != PackageManager.PERMISSION_GRANTED) {
                throw BlePermissionNotGrantedException()
            }

            it.startAdvertising(settings, data, scanResult, advertiseCallback)
        } ?: Log.w("InterShareSDK [BLE Manager]", "Failed to create advertiser")
    }

    private fun stopAdvertising() {
        if (ActivityCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_ADVERTISE) != PackageManager.PERMISSION_GRANTED) {
            throw BlePermissionNotGrantedException()
        }

        val bluetoothLeAdvertiser: BluetoothLeAdvertiser? = bluetoothManager.adapter.bluetoothLeAdvertiser
        bluetoothLeAdvertiser?.stopAdvertising(advertiseCallback) ?: Log.w("InterShareSDK [BLE Manager]", "Failed to create advertiser")
    }

    override fun startServer() {
        if (!bluetoothManager.adapter.isEnabled) {
            Log.d("InterShareSDK [BLE Manager]", "Bluetooth is currently disabled...enabling")
        } else {
            Log.d("InterShareSDK [BLE Manager]", "Bluetooth enabled...starting optimized services")
            startGattServer()
            startAdvertising()
        }
    }

    override fun stopServer() {
        stopAdvertising()
        stopGattServer()
    }
}
