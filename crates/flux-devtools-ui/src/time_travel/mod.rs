//! Time-travel data store (ADR-0042): the ring buffer plus the pure state
//! reconstruction used by the timeline scrubber.

mod buffer;
mod reconstruct;

pub use buffer::{DEFAULT_CAPACITY, TimelineBuffer};
pub use reconstruct::{ReconstructedState, Registers, reconstruct_state};
