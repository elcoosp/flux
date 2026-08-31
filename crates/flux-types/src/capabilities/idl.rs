/// One method on a capability: its wire name and numeric id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodIdl {
    /// The method name as written in `.flux` (e.g. `take`, `navigate`).
    pub name: &'static str,
    /// The numeric method id used by `CALL_CAP`.
    pub id: u16,
}

/// One capability: its wire name, numeric id, and methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityIdl {
    /// The capability name as written in `.flux` and advertised in `Hello`.
    pub name: &'static str,
    /// The numeric capability id used by `CALL_CAP`.
    pub id: u32,
    /// The methods this capability exposes.
    pub methods: &'static [MethodIdl],
}

impl CapabilityIdl {
    /// Resolves a numeric `(cap_id, method_id)` pair to its wire names.
    ///
    /// Returns `None` when the ids are not part of the MLP manifest — a program
    /// that `CALL_CAP`s an unknown id cannot be satisfied by any host.
    #[must_use]
    pub fn names_for(cap_id: u32, method_id: u16) -> Option<(&'static str, &'static str)> {
        let cap = CAPABILITY_IDL.iter().find(|c| c.id == cap_id)?;
        let method = cap.methods.iter().find(|m| m.id == method_id)?;
        Some((cap.name, method.name))
    }

    /// Resolves a capability name to its numeric id, or `None` if the name is
    /// not part of the MLP manifest.
    ///
    /// Used by the IR lower to emit the `CALL_CAP` `cap_id` from the capability
    /// identifier written in `.flux`, so the compiler and runtime stay in lock-
    /// step with [`CAPABILITY_IDL`].
    #[must_use]
    pub fn id_for(name: &str) -> Option<u32> {
        CAPABILITY_IDL.iter().find(|c| c.name == name).map(|c| c.id)
    }

    /// Resolves a `(capability_name, method_name)` pair to its numeric method id,
    /// or `None` if either name is not part of the MLP manifest.
    #[must_use]
    pub fn method_id_for(cap: &str, method: &str) -> Option<u16> {
        let cap = CAPABILITY_IDL.iter().find(|c| c.name == cap)?;
        cap.methods.iter().find(|m| m.name == method).map(|m| m.id)
    }
}

/// The MLP capability set (mirrors `stdlib/capabilities.flux`).
///
/// IDs are stable and match the native `CapabilityRegistry` tables (cap 1 =
/// Camera, cap 2 = Storage, cap 3 = Router, cap 4 = Clipboard, cap 5 =
/// Geolocation). Sync vs async is a binding detail: sync methods return
/// immediately; async methods (most platform calls — camera, permissions,
/// network) resolve through the VM's await machinery (ADR-0044 / ADR-0045) and
/// return a `Result` on failure.
pub const CAPABILITY_IDL: &[CapabilityIdl] = &[
    CapabilityIdl {
        name: "Camera",
        id: 1,
        methods: &[
            MethodIdl {
                name: "takePicture",
                id: 1,
            },
            MethodIdl {
                name: "startPreview",
                id: 2,
            },
            MethodIdl {
                name: "stopPreview",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "Storage",
        id: 2,
        methods: &[
            MethodIdl {
                name: "setItem",
                id: 1,
            },
            MethodIdl {
                name: "getItem",
                id: 2,
            },
            MethodIdl {
                name: "removeItem",
                id: 3,
            },
            // --- FLUX-078: the iOS host registers a `(2, 99)` reference async
            // capability (ADR-0045 result-cell demo). It was hand-assigned on the
            // host and never declared here, so its id was non-deterministic.
            // Declare it deterministically so both hosts and the server agree.
            MethodIdl {
                name: "devReferenceAsync",
                id: 99,
            },
        ],
    },
    CapabilityIdl {
        name: "Router",
        id: 3,
        methods: &[MethodIdl {
            name: "navigate",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Clipboard",
        id: 4,
        methods: &[
            MethodIdl {
                name: "setString",
                id: 1,
            },
            MethodIdl {
                name: "getString",
                id: 2,
            },
        ],
    },
    CapabilityIdl {
        name: "Geolocation",
        id: 5,
        methods: &[MethodIdl {
            name: "getCurrentPosition",
            id: 1,
        }],
    },
    // --- FLUX-045: six concrete native capabilities (PRD-Q deferred set) ---
    // IDs continue the stable sequence; native host bodies are wired in
    // `runtimes/android/host` + `runtimes/ios` (parallel-owned, not here).
    CapabilityIdl {
        name: "Push",
        id: 6,
        methods: &[
            MethodIdl {
                name: "registerForNotifications",
                id: 1,
            },
            MethodIdl {
                name: "scheduleNotification",
                id: 2,
            },
        ],
    },
    CapabilityIdl {
        name: "Biometric",
        id: 7,
        methods: &[MethodIdl {
            name: "authenticate",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Background",
        id: 8,
        methods: &[MethodIdl {
            name: "schedule",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "FileSystem",
        id: 9,
        methods: &[
            MethodIdl {
                name: "readAsString",
                id: 1,
            },
            MethodIdl {
                name: "writeAsString",
                id: 2,
            },
            MethodIdl {
                name: "delete",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "DeepLink",
        id: 10,
        methods: &[MethodIdl {
            name: "openURL",
            id: 1,
        }],
    },
    CapabilityIdl {
        name: "Sensors",
        id: 11,
        methods: &[MethodIdl {
            name: "read",
            id: 1,
        }],
    },
    // --- FLUX-048: WebView escape-hatch capability (release valve: embed web). ---
    // `load` (12,1) sets the webview `src` (a URL or html); `evaluate` (12,2)
    // runs JS in the page; `sendMessage` (12,3) posts to the host↔web bridge.
    CapabilityIdl {
        name: "WebView",
        id: 12,
        methods: &[
            MethodIdl {
                name: "load",
                id: 1,
            },
            MethodIdl {
                name: "evaluate",
                id: 2,
            },
            MethodIdl {
                name: "sendMessage",
                id: 3,
            },
        ],
    },
    // --- FLUX-046: native-module escape hatch (wrap any native SDK). ---
    // The host allow-lists which module names resolve (LANE-I); an undeclared
    // module is denied at the gate like any capability. `invoke` (13,1) calls a
    // method on the wrapped native module with positional args. Declared before
    // Http/Persist (ids 14/15) so the table follows id order.
    CapabilityIdl {
        name: "NativeModule",
        id: 13,
        methods: &[MethodIdl {
            name: "invoke",
            id: 1,
        }],
    },
    // --- FLUX-047: HTTP fetch/JSON + structured persistence capabilities. ---
    // `Http` performs async network requests (resolves through the VM's await
    // machinery, ADR-0044/0045); `Persist` is structured, queryable local
    // persistence beyond key-value `Storage` (a thin queryable store).
    CapabilityIdl {
        name: "Http",
        id: 14,
        methods: &[
            MethodIdl {
                name: "fetch",
                id: 1,
            },
            MethodIdl {
                name: "getJson",
                id: 2,
            },
            MethodIdl {
                name: "postJson",
                id: 3,
            },
        ],
    },
    CapabilityIdl {
        name: "Persist",
        id: 15,
        methods: &[
            MethodIdl { name: "put", id: 1 },
            MethodIdl { name: "get", id: 2 },
            MethodIdl {
                name: "query",
                id: 3,
            },
            MethodIdl {
                name: "delete",
                id: 4,
            },
        ],
    },
];
