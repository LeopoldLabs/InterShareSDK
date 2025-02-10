package com.julian_baumann.intershare_sdk.bluetoothLowEnergy

import android.annotation.SuppressLint
import com.julian_baumann.intershare_sdk.InternalNearbyServer
import com.julian_baumann.intershare_sdk.L2CapDelegate
import com.julian_baumann.intershare_sdk.handleIncomingL2capConnection
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class L2CAPClientManager(private val internalHandler: InternalNearbyServer): L2CapDelegate {
    @SuppressLint("MissingPermission")
    override fun openL2capConnection(connectionId: String, peripheralUuid: String, psm: UInt) {
        val peripheral = BLECentralManager.discoveredPeripherals.find { device -> device.address == peripheralUuid }

        if (peripheral == null) {
            return
        }

        val socket = peripheral.createInsecureL2capChannel(psm.toInt())
        socket.connect()
        val stream = L2CAPStream(socket)

        CoroutineScope(Dispatchers.IO).launch {
            handleIncomingL2capConnection(connectionId, stream)
        }
    }
}
