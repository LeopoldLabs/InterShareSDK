//
//  File.swift
//  
//
//  Created by Julian Baumann on 06.01.24.
//

import Foundation
import CoreBluetooth

let ServiceUUID = CBUUID.init(string: getBleServiceUuid())
let DiscoveryCharacteristicUUID = CBUUID.init(string: getBleDiscoveryCharacteristicUuid())
let WriteCharacteristicUUID = CBUUID.init(string: "6A70FCC9-C0D3-4AA1-9087-EFFC38576141")
