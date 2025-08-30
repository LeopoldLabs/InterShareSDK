//
//  Stream.swift
//
//
//  Created by Julian Baumann on 08.01.24.
//

import Foundation
import InterShareSDKFFI
import CoreBluetooth

class L2CapStream: NSObject, StreamDelegate, NativeStreamDelegate {
    private var channel: CBL2CAPChannel?

    init(channel: CBL2CAPChannel) {
        self.channel = channel
        super.init()

        // Open and schedule on the current run loop so hasBytesAvailable/hasSpaceAvailable update.
        channel.inputStream.schedule(in: .current, forMode: .default)
        channel.outputStream.schedule(in: .current, forMode: .default)
        channel.inputStream.open()
        channel.outputStream.open()
    }

    private func pumpRunLoop(_ interval: TimeInterval = 0.01) {
        _ = RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(interval))
    }

    func read(bufferLength: UInt64) -> Data {
        guard let ch = channel else { return Data() }
        let cap = Int(bufferLength)
        if cap <= 0 { return Data() }

        var out = Data()
        out.reserveCapacity(cap)

        // Block until we read at least 1 byte or hit EOF/error.
        while out.isEmpty {
            // If bytes are available, read once (don’t try to fill cap—that’s fine).
            if ch.inputStream.hasBytesAvailable {
                var buf = [UInt8](repeating: 0, count: cap)
                let n = ch.inputStream.read(&buf, maxLength: cap)

                if n > 0 {
                    out.append(buf, count: n)
                    break
                } else if n == 0 {
                    // 0 with .atEnd is EOF; otherwise just wait for more bytes.
                    if ch.inputStream.streamStatus == .atEnd { break }
                    pumpRunLoop()
                } else {
                    // n < 0 => error; propagate as EOF to let upper layer abort.
                    // (You can plumb an error channel if you prefer.)
                    break
                }
            } else {
                // No bytes yet; wait a tick.
                if ch.inputStream.streamStatus == .atEnd { break }
                pumpRunLoop()
            }
        }

        // print("\(out.count) bytes read via L2CAP")
        return out
    }

    func write(data: Data) -> UInt64 {
        guard let ch = channel, !data.isEmpty else { return 0 }
        var totalWritten = 0
        data.withUnsafeBytes { (rawBuf: UnsafeRawBufferPointer) in
            var ptr = rawBuf.bindMemory(to: UInt8.self).baseAddress!
            var remaining = data.count

            while remaining > 0 {
                if ch.outputStream.hasSpaceAvailable {
                    let n = ch.outputStream.write(ptr, maxLength: remaining)
                    if n > 0 {
                        totalWritten += n
                        ptr += n
                        remaining -= n
                    } else if n == 0 {
                        // Back-pressure; wait and retry
                        pumpRunLoop(0.005)
                    } else {
                        // n < 0 => error. Break; we'll return what we wrote so far.
                        break
                    }
                } else {
                    pumpRunLoop(0.005)
                }
            }
        }

        // print("\(totalWritten) bytes written via L2CAP")
        return UInt64(totalWritten)
    }

    func flush() {
        // Nothing special for CFStreams; pump once to let the OS drain buffers.
        pumpRunLoop(0.005)
    }

    func disconnect() {
        channel?.outputStream.close()
        channel?.inputStream.close()
        channel?.outputStream.remove(from: .current, forMode: .default)
        channel?.inputStream.remove(from: .current, forMode: .default)
        channel = nil
    }
}
