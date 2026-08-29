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
    // ===== GENERATED-BEGIN (derived from flux-devserver capability_idl; do not edit) =====
    private static let idlCapabilities: [(String, UInt32, [(String, UInt16)])] = [
        ("Camera", 1, [
            ("takePicture", 1),
            ("startPreview", 2),
            ("stopPreview", 3),
        ]),
        ("Storage", 2, [
            ("setItem", 1),
            ("getItem", 2),
            ("removeItem", 3),
        ]),
        ("Router", 3, [
            ("navigate", 1),
        ]),
        ("Clipboard", 4, [
            ("setString", 1),
            ("getString", 2),
        ]),
        ("Geolocation", 5, [
            ("getCurrentPosition", 1),
        ]),
        ("Push", 6, [
            ("registerForNotifications", 1),
            ("scheduleNotification", 2),
        ]),
        ("Biometric", 7, [
            ("authenticate", 1),
        ]),
        ("Background", 8, [
            ("schedule", 1),
        ]),
        ("FileSystem", 9, [
            ("readAsString", 1),
            ("writeAsString", 2),
            ("delete", 3),
        ]),
        ("DeepLink", 10, [
            ("openURL", 1),
        ]),
        ("Sensors", 11, [
            ("read", 1),
        ]),
        ("WebView", 12, [
            ("load", 1),
            ("evaluate", 2),
            ("sendMessage", 3),
        ]),
        ("NativeModule", 13, [
            ("invoke", 1),
        ]),
    ]
    // ===== GENERATED-END =====

    /// The capabilities this host build advertises (Appendix D §D.12.1, §24.4).
    ///
    /// Each entry is `(name, version, features)`. The dev server validates the
    /// set against the compiled `.flux` requirements; a mismatch is a clear
    /// `Error` frame rather than a silent runtime fault. The ids/names here
    /// are generated from the framework's capability IDL and match the native
    /// `CapabilityRegistry` table and `stdlib/capabilities.flux`.
    static let advertisedCapabilities: [(String, UInt32, [String])] = idlCapabilities.map {
        ($0.0, $0.1, $0.2.map { $0.0 })
    }
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
            // Capability `version` is encoded as `u32` LE on the wire (matches
            // `flux_ir_serde::HelloFrame::to_bytes`/`from_hello_bytes`, which read
            // it with `read_u32`). Encoding it as `u16` here desyncs the whole
            // capability tuple and makes the server reject the handshake as
            // malformed ("expected a Hello frame") — which surfaces as a blank
            // screen on the host.
            data.append(UInt8(version & 0xFF))
            data.append(UInt8((version >> 8) & 0xFF))
            data.append(UInt8((version >> 16) & 0xFF))
            data.append(UInt8((version >> 24) & 0xFF))
            data.append(UInt8(features.count & 0xFF))
            data.append(UInt8((features.count >> 8) & 0xFF))
            for feature in features { data.fluxAppendString(feature) }
        }
        return data
    }
}
