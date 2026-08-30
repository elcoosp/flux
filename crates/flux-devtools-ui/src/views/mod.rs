//! DevTools views (spec §5.1, §5.3). Each view renders a facet of the
//! [`DevToolsState`] and is owned by the gpui root window.

mod component_tree;
mod log_viewer;
mod network_inspector;
mod signal_graph;
mod timeline;
mod vm_inspector;

pub use component_tree::ComponentTreeView;
pub use log_viewer::LogViewerView;
pub use network_inspector::NetworkInspectorView;
pub use signal_graph::SignalGraphView;
pub use timeline::TimelineView;
pub use vm_inspector::VmInspectorView;
