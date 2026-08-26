//! DevTools views (spec §5.1, §5.3). Each view renders a facet of the
//! [`DevToolsState`] and is owned by the gpui root window.

mod component_tree;
mod signal_graph;
mod timeline;
mod vm_inspector;

pub use component_tree::ComponentTreeView;
pub use signal_graph::SignalGraphView;
pub use timeline::TimelineView;
pub use vm_inspector::VmInspectorView;
