//! Live component-instance tracking (Appendix C §C.2).
//!
//! When the dev server ships a tree to the host app, the host materialises a
//! [`ComponentInstance`] per live component node. The [`InstanceRegistry`]
//! maps both `InstanceId` and the originating [`NodeId`] to each instance, so a
//! hot-swapped tree can preserve signal state and effects across edits (ASR-003).

pub use component_instance::ComponentInstance;
pub use registry::InstanceRegistry;

mod component_instance;
mod registry;

#[cfg(test)]
mod tests;
