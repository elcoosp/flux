//  HelloFrame.swift
//  Dev-mode wire handshake (Appendix D §D.12.1).
//
//  The dev server requires the host to send a `Hello` frame before it replies
//  with the full-tree `Init`. The byte layout matches
//  `flux_ir_serde::HelloFrame::to_bytes`:
//
//      MAGIC(u32 LE = 0x465C5558) | version(u8 = 1) | kind(u8 = 0x01)
//        | platform(u16 len + utf8) | device(u16 len + utf8)
//        | cap_count(u16) [cap triples…]
//
//  Sending this on socket open is what lets the server answer with the tree;
//  without it the connection hangs (the server only fans out `Init` after a
//  valid `Hello`).

import Foundation

extension Data {
    /// Appends a `u16` length-prefixed UTF-8 string (Appendix D §D.9).
    public mutating func fluxAppendString(_ s: String) {
        let bytes = Data(s.utf8)
        self.append(UInt8(bytes.count & 0xFF))
        self.append(UInt8((bytes.count >> 8) & 0xFF))
        self.append(bytes)
    }
}

extension HelloFrame {
    /// The capabilities this host build advertises (Appendix D §D.12.1, §24.4).
    ///
    /// Each entry is `(name, version, features)`. The dev server validates the
    /// set against the compiled `.flux` requirements; a mismatch is a clear
    /// `Error` frame rather than a silent runtime fault. The ids here match the
    /// stable capability ids from `stdlib/capabilities.flux` / the native
    /// `CapabilityRegistry` (cap 1 = Camera, cap 2 = Storage, cap 3 = Router).
    static let advertisedCapabilities: [(String, UInt32, [String])] = [
        ("Camera", 1, ["take", "startPreview", "stopPreview"]),
        ("Storage", 1, ["set", "get", "delete"]),
        ("Router", 1, ["navigate"]),
    ]
}

public enum HelloFrame {
    /// Builds the wire bytes of a `Hello` handshake frame.
    /// - Parameters:
    ///   - platform: host platform string, e.g. "ios".
    ///   - device: device model string, e.g. "iPhone 17 Pro".
    /// - Returns: the frame bytes to send over the WebSocket.
    public static func bytes(platform: String, device: String) -> Data {
        var data = Data()
        // MAGIC "FLUX" little-endian (u32 = 0x465C5558): 0x58 0x55 0x5C 0x46.
        data.append(0x58)
        data.append(0x55)
        data.append(0x5C)
        data.append(0x46)
        data.append(1) // protocol version
        data.append(0x01) // FrameKind::Hello
        data.fluxAppendString(platform)
        data.fluxAppendString(device)
        // cap_count (u16 LE).
        let caps = advertisedCapabilities
        data.append(UInt8(caps.count & 0xFF))
        data.append(UInt8((caps.count >> 8) & 0xFF))
        for (name, version, features) in caps {
            data.fluxAppendString(name)
            data.append(UInt8(version & 0xFF))
            data.append(UInt8((version >> 8) & 0xFF))
            data.append(UInt8(features.count & 0xFF))
            data.append(UInt8((features.count >> 8) & 0xFF))
            for feature in features { data.fluxAppendString(feature) }
        }
        return data
    }
}
