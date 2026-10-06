//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;

pub use car::CarSpec;
