use super::capability_error::CapabilityError;
use super::compile_err::{CompileError, CompilePhase};
use super::flux_error::FluxError;
use flux_syntax::Span;

/// Builds a denied-capability [`FluxError`] from the capability/method ids and
/// the permission token that was missing.
///
/// `cap_id` / `method_id` are the raw `CALL_CAP` operands; the IDL names are
/// resolved by the caller (or left `None`) so the host can display a precise
/// red banner.
#[must_use]
pub fn capability_denied(
    cap_id: u32,
    method_id: u16,
    cap_name: Option<String>,
    method_name: Option<String>,
    required_permission: String,
) -> FluxError {
    let why = format!("required permission `{required_permission}` was not granted");
    FluxError::Capability(CapabilityError {
        why,
        cap_id,
        cap_name,
        method_id,
        method_name,
        required_permission,
    })
}

/// Builds a [`FluxError::Compile`] carrying message, span, optional hint and phase.
#[must_use]
pub fn compile_error(
    message: impl Into<String>,
    span: Span,
    hint: Option<String>,
    phase: CompilePhase,
) -> FluxError {
    FluxError::Compile(CompileError {
        message: message.into(),
        span,
        hint,
        phase,
    })
}
