//  FluxUIKit.swift
//  FluxUIKit — Swift adapter kit entry (FLUX-008).
//
//  The kit's public surface is the contract the dev runtime (FLUX-006)
//  consumes: `FluxValue`, `Props`, `FluxColor`/`FluxFount`/`FluxAlignment`,
//  `FluxEvent`, `FluxExecutor`, and the `FluxAdapter` protocol, plus the nine
//  declarative adapters (Text, Button, Column, Row, TextField, Image, Container,
//  Router, Screen).

/// The adapter contract version this kit implements (Appendix F).
public enum FluxUIKit {
    /// The adapter contract version this kit implements (Appendix F).
    public static let adapterContractVersion = 1
}
