//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;
mod sim;

pub use car::CarSpec;
pub use sim::{Controls, Env, Event, RPM_PER_RAD_S, SHIFT_CLUTCH, Sim, SimState};
