//! Vortex's isolated integration boundary around Servo.
//!
//! Keep Servo's Git revision and feature selection in this crate only.
//! Browser-shell code should depend on this crate rather than declaring Servo
//! directly. This transitional facade re-exports Servo's API; later work can
//! narrow it to Vortex-owned interfaces without changing the dependency owner.

pub use servo;

/// Exact upstream Servo revision pinned for reproducible builds.
pub const SERVO_REVISION: &str = "32fc710d5b0eb4bd27d4f3b5317a0f5f97b3bf17";

/// Version of the Vortex engine boundary contract.
pub const ENGINE_BOUNDARY_VERSION: u32 = 1;
