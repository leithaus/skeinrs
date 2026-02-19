//! Gesture recognition — both from LeapMotion hardware and keyboard simulation.
//!
//! The public interface is [`GestureEvent`] delivered over a `mpsc` channel.
//! Consumers don't need to know whether events came from real hardware or the
//! keyboard simulator.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

// ════════════════════════════════════════════════════════════════════════════
// GestureEvent
// ════════════════════════════════════════════════════════════════════════════

/// A high-level gesture emitted by the recogniser.
#[derive(Clone, Debug, PartialEq)]
pub enum GestureEvent {
    /// Pull Left hand — advance the Left (duration) stream by `steps` digits.
    /// `velocity` is 0.0–1.0 (normalised pull speed).
    PullLeft  { steps: usize, velocity: f32 },

    /// Pull Right hand — advance the Right (pitch) stream.
    PullRight { steps: usize, velocity: f32 },

    /// Twist: one hand crossed over the other → swap Left/Right.
    Twist,

    /// Both hands brought together → begin MIDI playback.
    Clap,

    /// Hands separated after a clap → stop MIDI playback.
    Unclap,

    /// Scissors gesture on either hand → invoke snip.
    /// The `name` is collected interactively from the user.
    Scissors { name: String },

    /// Quit the application.
    Quit,
}

// ════════════════════════════════════════════════════════════════════════════
// GestureSource trait — unified interface for hw and sim
// ════════════════════════════════════════════════════════════════════════════

/// Anything that can deliver [`GestureEvent`]s over a channel.
pub trait GestureSource: Send + 'static {
    fn run(self: Box<Self>, tx: Sender<GestureEvent>);
}

// ════════════════════════════════════════════════════════════════════════════
// Spawn helper
// ════════════════════════════════════════════════════════════════════════════

/// Spawn a gesture source on its own thread and return the receiving end.
pub fn spawn_gesture_source<G: GestureSource>(source: G) -> Receiver<GestureEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || Box::new(source).run(tx));
    rx
}

// ════════════════════════════════════════════════════════════════════════════
// LeapGestureSource — real hardware (feature = "leap")
// ════════════════════════════════════════════════════════════════════════════

/// Gesture source backed by a real LeapMotion controller.
///
/// Requires the `leap` feature flag and the LeapC shared library installed.
///
/// # Algorithm
///
/// Each polling frame we examine hand palm positions and velocities:
///
/// * **Pull**: palm velocity along the Z-axis (toward camera) > threshold.
///   Steps = floor(|vz| / STEP_DIVISOR), capped to avoid jumps.
/// * **Twist**: left-hand palm Y > right-hand palm Y (left over right) or
///   vice-versa, sustained for TWIST_FRAMES consecutive frames.
/// * **Clap**: inter-palm distance < CLAP_DIST_MM and decreasing.
/// * **Unclap**: inter-palm distance > UNCLAP_DIST_MM after a clap.
/// * **Scissors**: index and middle fingers extended, others curled,
///   with spread angle > SCISSORS_ANGLE_DEG, sustained for SCISSORS_FRAMES.
#[cfg(feature = "leap")]
pub struct LeapGestureSource;

#[cfg(feature = "leap")]
impl GestureSource for LeapGestureSource {
    fn run(self: Box<Self>, tx: Sender<GestureEvent>) {
        use leaprs::*;

        // Thresholds (empirically tuned)
        const CLAP_DIST:       f32 = 80.0;   // mm — hands this close = clap
        const UNCLAP_DIST:     f32 = 150.0;  // mm — hands this far  = unclap
        const PULL_VZ_MIN:     f32 = 150.0;  // mm/s — minimum pull velocity
        const STEP_DIVISOR:    f32 = 100.0;  // mm/s per step
        const SCISSORS_ANGLE:  f32 = 25.0;   // degrees between index/middle
        const TWIST_HOLD:      u32 = 6;      // frames to confirm twist
        const SCISSORS_HOLD:   u32 = 4;      // frames to confirm scissors
        const PULL_COOLDOWN:   Duration = Duration::from_millis(80);
        const SCISSORS_COOLDOWN: Duration = Duration::from_millis(500);

        let mut connection = Connection::create(ConnectionConfig::default())
            .expect("Failed to open LeapC connection");
        connection.open().expect("Failed to open LeapMotion device");

        let mut clappped      = false;
        let mut twist_counter = 0u32;
        let mut scissors_l    = 0u32;
        let mut scissors_r    = 0u32;
        let mut last_pull_l   = Instant::now() - PULL_COOLDOWN;
        let mut last_pull_r   = Instant::now() - PULL_COOLDOWN;
        let mut last_scissors = Instant::now() - SCISSORS_COOLDOWN;

        loop {
            let msg = match connection.poll(100) {
                Ok(m)  => m,
                Err(_) => continue,
            };

            if let Event::Tracking(frame) = msg.event() {
                let hands: Vec<_> = frame.hands().collect();
                if hands.is_empty() { continue; }

                // ── separate left/right ───────────────────────────────────
                let left  = hands.iter().find(|h| h.hand_type() == HandType::Left);
                let right = hands.iter().find(|h| h.hand_type() == HandType::Right);

                // ── Clap / Unclap ─────────────────────────────────────────
                if let (Some(lh), Some(rh)) = (left, right) {
                    let lp = lh.palm().position();
                    let rp = rh.palm().position();
                    let dx = lp.x - rp.x;
                    let dy = lp.y - rp.y;
                    let dz = lp.z - rp.z;
                    let dist = (dx*dx + dy*dy + dz*dz).sqrt();

                    if !clappped && dist < CLAP_DIST {
                        clappped = true;
                        let _ = tx.send(GestureEvent::Clap);
                    } else if clappped && dist > UNCLAP_DIST {
                        clappped = false;
                        let _ = tx.send(GestureEvent::Unclap);
                    }

                    // ── Twist ─────────────────────────────────────────────
                    // Left hand Y > Right hand Y means left is "over" right.
                    let lh_over_rh = lp.y > rp.y + 40.0;
                    let rh_over_lh = rp.y > lp.y + 40.0;
                    if lh_over_rh || rh_over_lh {
                        twist_counter += 1;
                        if twist_counter == TWIST_HOLD {
                            let _ = tx.send(GestureEvent::Twist);
                        }
                    } else {
                        twist_counter = 0;
                    }
                } else {
                    twist_counter = 0;
                }

                // ── Pull Left ─────────────────────────────────────────────
                if let Some(lh) = left {
                    let vz = lh.palm().velocity().z;
                    if vz > PULL_VZ_MIN && last_pull_l.elapsed() > PULL_COOLDOWN {
                        last_pull_l = Instant::now();
                        let steps = ((vz / STEP_DIVISOR) as usize).max(1).min(20);
                        let vel   = (vz / 600.0).min(1.0);
                        let _ = tx.send(GestureEvent::PullLeft { steps, velocity: vel });
                    }
                    // Scissors on left hand
                    if is_scissors(lh) {
                        scissors_l += 1;
                        if scissors_l == SCISSORS_HOLD
                            && last_scissors.elapsed() > SCISSORS_COOLDOWN
                        {
                            last_scissors = Instant::now();
                            let name = prompt_snippet_name();
                            let _ = tx.send(GestureEvent::Scissors { name });
                        }
                    } else {
                        scissors_l = 0;
                    }
                }

                // ── Pull Right ────────────────────────────────────────────
                if let Some(rh) = right {
                    let vz = rh.palm().velocity().z;
                    if vz > PULL_VZ_MIN && last_pull_r.elapsed() > PULL_COOLDOWN {
                        last_pull_r = Instant::now();
                        let steps = ((vz / STEP_DIVISOR) as usize).max(1).min(20);
                        let vel   = (vz / 600.0).min(1.0);
                        let _ = tx.send(GestureEvent::PullRight { steps, velocity: vel });
                    }
                    // Scissors on right hand
                    if is_scissors(rh) {
                        scissors_r += 1;
                        if scissors_r == SCISSORS_HOLD
                            && last_scissors.elapsed() > SCISSORS_COOLDOWN
                        {
                            last_scissors = Instant::now();
                            let name = prompt_snippet_name();
                            let _ = tx.send(GestureEvent::Scissors { name });
                        }
                    } else {
                        scissors_r = 0;
                    }
                }
            }
        }
    }
}

/// Returns true if the hand shows a scissors gesture:
/// index + middle extended and spread, ring + pinky curled.
#[cfg(feature = "leap")]
fn is_scissors(hand: &leaprs::Hand) -> bool {
    use leaprs::FingerType::*;
    const EXTENDED_ANGLE: f32 = 0.4; // radians — finger considered "straight"
    const SPREAD_ANGLE:   f32 = 0.35;

    let fingers: Vec<_> = hand.digits().collect();
    if fingers.len() < 5 { return false; }

    let index_ext  = finger_extension(&fingers[1]) > EXTENDED_ANGLE;
    let middle_ext = finger_extension(&fingers[2]) > EXTENDED_ANGLE;
    let ring_curl  = finger_extension(&fingers[3]) < 0.2;
    let pinky_curl = finger_extension(&fingers[4]) < 0.2;

    if !(index_ext && middle_ext && ring_curl && pinky_curl) {
        return false;
    }

    // Check spread between index and middle tip directions
    let ib = fingers[1].distal().next_joint();
    let mb = fingers[2].distal().next_joint();
    let ip = fingers[1].distal().prev_joint();
    let mp = fingers[2].distal().prev_joint();

    let id = [ib.x - ip.x, ib.y - ip.y, ib.z - ip.z];
    let md = [mb.x - mp.x, mb.y - mp.y, mb.z - mp.z];

    let dot    = id[0]*md[0] + id[1]*md[1] + id[2]*md[2];
    let il     = (id[0]*id[0]+id[1]*id[1]+id[2]*id[2]).sqrt();
    let ml     = (md[0]*md[0]+md[1]*md[1]+md[2]*md[2]).sqrt();
    if il < 1e-6 || ml < 1e-6 { return false; }
    let cos_a  = (dot / (il * ml)).clamp(-1.0, 1.0);
    let angle  = cos_a.acos();
    angle > SPREAD_ANGLE
}

#[cfg(feature = "leap")]
fn finger_extension(digit: &leaprs::Digit) -> f32 {
    // Ratio of (tip – metacarpal base) distance to full finger length.
    // 1.0 = fully extended, ~0.0 = fully curled.
    let base = digit.metacarpal().prev_joint();
    let tip  = digit.distal().next_joint();
    let dx   = tip.x - base.x;
    let dy   = tip.y - base.y;
    let dz   = tip.z - base.z;
    let dist = (dx*dx + dy*dy + dz*dz).sqrt();
    // Normalise to ~0–1 using typical finger length ≈ 80 mm
    (dist / 80.0).clamp(0.0, 1.0)
}

// ════════════════════════════════════════════════════════════════════════════
// SimGestureSource — keyboard/mouse simulation (always available)
// ════════════════════════════════════════════════════════════════════════════

/// Gesture source driven by [`SimInput`] events (from the visualizer's window).
///
/// The visualizer sends `SimInput` events here; this translator converts them
/// to `GestureEvent`s.  This decouples the window event loop from gesture
/// logic.
pub struct SimGestureSource {
    pub rx: std::sync::mpsc::Receiver<SimInput>,
}

/// Raw input event from the simulation window.
#[derive(Clone, Debug)]
pub enum SimInput {
    KeyDown(SimKey),
    KeyUp(SimKey),
    /// Snippet name typed by the user after a scissors key press.
    SnippetName(String),
}

/// Simulated key codes (mapped from minifb Key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimKey {
    PullLeft,       // A
    PullRight,      // D
    PullLeftFast,   // Shift+A
    PullRightFast,  // Shift+D
    Twist,          // T
    Clap,           // Space
    Unclap,         // Escape
    Scissors,       // S
    Quit,           // Q
}

impl GestureSource for SimGestureSource {
    fn run(self: Box<Self>, tx: Sender<GestureEvent>) {
        for input in self.rx {
            let event = match input {
                SimInput::KeyDown(SimKey::PullLeft)      =>
                    GestureEvent::PullLeft  { steps: 1,  velocity: 0.3 },
                SimInput::KeyDown(SimKey::PullLeftFast)  =>
                    GestureEvent::PullLeft  { steps: 5,  velocity: 0.9 },
                SimInput::KeyDown(SimKey::PullRight)     =>
                    GestureEvent::PullRight { steps: 1,  velocity: 0.3 },
                SimInput::KeyDown(SimKey::PullRightFast) =>
                    GestureEvent::PullRight { steps: 5,  velocity: 0.9 },
                SimInput::KeyDown(SimKey::Twist)         => GestureEvent::Twist,
                SimInput::KeyDown(SimKey::Clap)          => GestureEvent::Clap,
                SimInput::KeyDown(SimKey::Unclap)        => GestureEvent::Unclap,
                SimInput::SnippetName(name)              =>
                    GestureEvent::Scissors { name },
                SimInput::KeyDown(SimKey::Quit)          => {
                    let _ = tx.send(GestureEvent::Quit);
                    return;
                }
                _ => continue,
            };
            if tx.send(event).is_err() { return; }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Shared snippet-name prompt (used by LeapGestureSource in hw mode)
// ════════════════════════════════════════════════════════════════════════════

/// Prompt the user for a snippet name on stdout/stdin.
/// In hardware mode this briefly pauses gesture recognition.
pub fn prompt_snippet_name() -> String {
    use std::io::{self, Write};
    print!("\n  Snippet name: ");
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
    buf.trim().to_string()
}
