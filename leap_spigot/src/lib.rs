//! # leap_spigot
//!
//! LeapMotion gesture controller for the dual transcendental spigot streams,
//! with real-time MIDI playback and a ribbon visualizer.
//!
//! ## Gesture → Action mapping
//!
//! | Gesture | Hand | Action |
//! |---|---|---|
//! | Pull toward body | Left | Advance Left (duration) stream; speed ∝ pull velocity |
//! | Pull toward body | Right | Advance Right (pitch) stream; speed ∝ pull velocity |
//! | Left hand over Right | Either | `twist()` — swap streams |
//! | Right hand over Left | Either | `twist()` — swap streams |
//! | Clap (hands together) | Both | Begin MIDI playback from current zip position |
//! | Un-clap (hands apart) | Both | Stop MIDI playback |
//! | Scissors (index+middle spread) | Either | Invoke `snip()` — user types key name |
//!
//! ## Visualization
//!
//! Two horizontal ribbons of colored digit-patches scroll left as the stream
//! advances.  When playing, the ribbons animate toward each other and are
//! "stitched" with a connecting thread.  Scissors/snip highlights a section
//! in gold and deposits it into the **Snippet Tray** on the right side.
//!
//! ## Feature flags
//!
//! * (default) — **Simulation mode**: keyboard shortcuts drive all gestures.
//! * `leap` — **Hardware mode**: polls a real LeapMotion controller via LeapC.
//!
//! ### Simulation keyboard shortcuts
//!
//! | Key | Gesture |
//! |---|---|
//! | `A` / hold | Pull Left stream (faster with Shift) |
//! | `D` / hold | Pull Right stream (faster with Shift) |
//! | `T` | Twist |
//! | `Space` | Clap / start MIDI |
//! | `Escape` | Un-clap / stop MIDI |
//! | `S` | Scissors / snip |
//! | `Q` | Quit |

pub mod gesture;
pub mod ribbon;
pub mod player;
pub mod visualizer;
pub mod app;
