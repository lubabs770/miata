//! Pure-Rust manual-transmission car model. No Bevy; the game drives it via
//! `Sim::step` and reads `SimState`.
mod car;
mod lesson;
mod score;
mod sim;

pub use car::CarSpec;
pub use lesson::{LessonId, LessonRun, Outcome};
pub use score::{Cup, stars};
pub use sim::{Controls, Env, Event, RPM_PER_RAD_S, SHIFT_CLUTCH, Sim, SimState};
