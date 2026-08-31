/// A denied OS permission (or an unknown capability id) surfaces as this variant
/// of [`FluxError`]; it never reaches native code and never panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityError {
    /// Numeric capability id that was invoked.
    pub cap_id: u32,
    /// Human capability name, when one is registered.
    pub cap_name: Option<String>,
    /// Numeric method id that was invoked.
    pub method_id: u16,
    /// Human method name, when one is registered.
    pub method_name: Option<String>,
    /// The OS permission token that was required (e.g. `.camera`).
    pub required_permission: String,
    /// The human-readable reason the grant was denied.
    pub why: String,
}
