//! The dev server's capability validation manifest (spec §24, ADR-0045).
//!
//! This is a thin adapter over the authoritative [`crate::capability_idl`]
//! table: all capability names, ids and method ids live in one place
//! (`capability_idl::CAPABILITY_IDL`) and are generated into every runtime plus
//! this manifest. The functions here exist so the `Hello`-handshake validator
//! (`crate::server::session`) has a small, focused surface — name↔id
//! resolution and advertised-set satisfaction — without depending on the IDL's
//! shape directly.

pub(crate) use crate::capability_idl::CapabilityIdl;
pub(crate) use crate::capability_idl::is_satisfied;

#[cfg(test)]
pub(crate) use crate::capability_idl::hello_capabilities;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_ids_match_native_registries() {
        // The ids here must equal the native registry tables and
        // stdlib/capabilities.flux (delegated to the IDL).
        assert_eq!(
            CapabilityIdl::names_for(1, 1),
            Some(("Camera", "takePicture"))
        );
        assert_eq!(CapabilityIdl::names_for(2, 2), Some(("Storage", "getItem")));
        assert_eq!(CapabilityIdl::names_for(3, 1), Some(("Router", "navigate")));
        assert_eq!(CapabilityIdl::names_for(9, 9), None);
    }

    #[test]
    fn satisfaction_checks_name_and_feature() {
        let advertised = hello_capabilities();
        assert!(is_satisfied(&advertised, "Camera", "takePicture"));
        assert!(is_satisfied(&advertised, "Storage", "getItem"));
        assert!(is_satisfied(&advertised, "Router", "navigate"));
        assert!(is_satisfied(&advertised, "Camera", "stopPreview"));
        // A method the host does not advertise must fail.
        let empty: Vec<(String, u32, Vec<String>)> = Vec::new();
        assert!(!is_satisfied(&empty, "Camera", "takePicture"));
    }
}
