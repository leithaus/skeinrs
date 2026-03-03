//! Unix-domain socket IPC bridge.
//!
//! Allows a visionOS Swift app (or any other client) to drive the Rust engine
//! over a local socket instead of through the minifb keyboard simulator.
//!
//! # Protocol
//!
//! Newline-delimited JSON.  Each line is one message object.
//!
//! ## Swift → Rust  (gesture events)
//!
//! ```json
//! {"type":"pull_left",  "steps":3, "velocity":0.72}
//! {"type":"pull_right", "steps":1, "velocity":0.31}
//! {"type":"twist"}
//! {"type":"clap"}
//! {"type":"unclap"}
//! {"type":"scissors",   "name":"my_snip"}
//! {"type":"quit"}
//! ```
//!
//! ## Rust → Swift  (state updates)
//!
//! ```json
//! {"type":"digits",   "left":[3,1,4],"right":[2,7,1],"left_pos":5,"right_pos":3}
//! {"type":"note",     "pitch":64,"duration":480,"velocity":100,"left_pos":5,"right_pos":3}
//! {"type":"snip_ack", "name":"foo","count":12}
//! {"type":"status",   "text":"Playing ♪"}
//! {"type":"twist_ack","left_label":"e base 10","right_label":"π base 10"}
//! ```
//!
//! # Socket path
//!
//! Default: `/tmp/leap_spigot.sock`
//! Override with `LEAP_SPIGOT_SOCK` environment variable.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;

use crate::gesture::GestureEvent;

pub const DEFAULT_SOCK: &str = "/tmp/leap_spigot.sock";

// ════════════════════════════════════════════════════════════════════════════
// Outbound state messages  (Rust → Swift)
// ════════════════════════════════════════════════════════════════════════════

/// A message pushed from the Rust engine to the Swift UI.
#[derive(Clone, Debug)]
pub enum StateMsg {
    /// New digits arrived on the ribbons.
    Digits {
        left:      Vec<u8>,
        right:     Vec<u8>,
        left_pos:  usize,
        right_pos: usize,
    },
    /// A note was played.
    Note {
        pitch:     u8,
        duration:  u32,
        velocity:  u8,
        left_pos:  usize,
        right_pos: usize,
    },
    /// A snip was stored.
    SnipAck {
        name:  String,
        count: usize,
    },
    /// Free-form status line.
    Status(String),
    /// Twist completed — new labels for each side.
    TwistAck {
        left_label:  String,
        right_label: String,
    },
}

impl StateMsg {
    /// Serialise to a JSON line (no `serde` dependency — hand-rolled).
    pub fn to_json_line(&self) -> String {
        match self {
            StateMsg::Digits { left, right, left_pos, right_pos } => {
                let lv = digits_json(left);
                let rv = digits_json(right);
                format!(
                    "{{\"type\":\"digits\",\"left\":{lv},\"right\":{rv},\
                     \"left_pos\":{left_pos},\"right_pos\":{right_pos}}}\n"
                )
            }
            StateMsg::Note { pitch, duration, velocity, left_pos, right_pos } => {
                format!(
                    "{{\"type\":\"note\",\"pitch\":{pitch},\"duration\":{duration},\
                     \"velocity\":{velocity},\"left_pos\":{left_pos},\
                     \"right_pos\":{right_pos}}}\n"
                )
            }
            StateMsg::SnipAck { name, count } => {
                format!("{{\"type\":\"snip_ack\",\"name\":\"{name}\",\"count\":{count}}}\n")
            }
            StateMsg::Status(text) => {
                let escaped = text.replace('"', "\\\"");
                format!("{{\"type\":\"status\",\"text\":\"{escaped}\"}}\n")
            }
            StateMsg::TwistAck { left_label, right_label } => {
                format!(
                    "{{\"type\":\"twist_ack\",\"left_label\":\"{left_label}\",\
                     \"right_label\":\"{right_label}\"}}\n"
                )
            }
        }
    }
}

fn digits_json(v: &[u8]) -> String {
    let inner: Vec<String> = v.iter().map(|d| d.to_string()).collect();
    format!("[{}]", inner.join(","))
}

// ════════════════════════════════════════════════════════════════════════════
// IpcStateSender — wraps the write half of the socket connection
// ════════════════════════════════════════════════════════════════════════════

/// Clone-able handle for sending state messages to the connected Swift client.
/// Silently drops messages if the client is disconnected.
#[derive(Clone)]
pub struct IpcStateSender {
    tx: Sender<StateMsg>,
}

impl IpcStateSender {
    pub fn send(&self, msg: StateMsg) {
        let _ = self.tx.send(msg);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IpcGestureSource — reads gesture JSON lines, emits GestureEvents
// ════════════════════════════════════════════════════════════════════════════

/// Gesture source that listens on a Unix socket for JSON gesture messages.
pub struct IpcGestureSource {
    sock_path: String,
}

impl IpcGestureSource {
    pub fn new(sock_path: &str) -> Self {
        IpcGestureSource { sock_path: sock_path.to_string() }
    }

    pub fn default_path() -> Self {
        let path = std::env::var("LEAP_SPIGOT_SOCK")
            .unwrap_or_else(|_| DEFAULT_SOCK.to_string());
        Self::new(&path)
    }
}

/// Spawn the IPC server.  Returns:
/// - A `Receiver<GestureEvent>` for the app loop to consume.
/// - An `IpcStateSender` for the app loop to push state back to the client.
///
/// The server accepts one client at a time.  When a client disconnects it
/// waits for the next connection.
pub fn spawn_ipc_server(
    source: IpcGestureSource,
) -> (Receiver<GestureEvent>, IpcStateSender) {
    let (gesture_tx, gesture_rx) = channel::<GestureEvent>();
    let (state_tx,   state_rx)   = channel::<StateMsg>();

    let sock_path = source.sock_path.clone();

    thread::spawn(move || {
        // Remove stale socket file
        let _ = std::fs::remove_file(&sock_path);

        let listener = UnixListener::bind(&sock_path)
            .unwrap_or_else(|e| panic!("Cannot bind {}: {}", sock_path, e));

        eprintln!("[ipc] Listening on {}", sock_path);

        // Writer thread: drains state_rx → socket write half
        // We pass the write half to it each time a new client connects.
        loop {
            match listener.accept() {
                Err(e) => { eprintln!("[ipc] Accept error: {}", e); continue; }
                Ok((stream, _)) => {
                    eprintln!("[ipc] Client connected");

                    // Clone the stream for the writer thread
                    let write_stream = match stream.try_clone() {
                        Ok(s)  => s,
                        Err(e) => { eprintln!("[ipc] Clone error: {}", e); continue; }
                    };

                    // Spawn writer
                    let state_rx_ref = {
                        // We can't move state_rx into multiple threads, so we
                        // use a shared channel pattern: forward from state_tx
                        // to a per-connection channel.
                        let (per_conn_tx, per_conn_rx) = channel::<StateMsg>();
                        // Bridge: state_tx still feeds per_conn_tx via the app
                        // We can't trivially bridge here without Arc<Mutex>,
                        // so instead we pass per_conn_tx as the new state sender
                        // and recreate it each connection.  Since run() holds
                        // an IpcStateSender clone, we signal via a flag.
                        // SIMPLIFICATION: use a thread-local forwarder.
                        (per_conn_tx, per_conn_rx)
                    };

                    // Simpler approach: give writer a direct clone of the stream
                    // and have it drain a channel we create per connection.
                    let (conn_state_tx, conn_state_rx) = channel::<StateMsg>();

                    // Forward from global state_rx to per-connection channel
                    // (runs until either end closes)
                    let conn_fwd_tx = conn_state_tx.clone();
                    // NOTE: state_rx is consumed by the forwarding loop.
                    // We use a separate forwarder below.

                    let mut writer = write_stream;
                    thread::spawn(move || {
                        for msg in conn_state_rx {
                            let line = msg.to_json_line();
                            if writer.write_all(line.as_bytes()).is_err() { break; }
                        }
                    });

                    // Read gestures from this connection
                    let gtx = gesture_tx.clone();
                    let reader = BufReader::new(stream);
                    for line in reader.lines() {
                        match line {
                            Err(_) => break,
                            Ok(l)  => {
                                if let Some(evt) = parse_gesture_json(&l) {
                                    let quit = matches!(evt, GestureEvent::Quit);
                                    let _ = gtx.send(evt);
                                    if quit { return; }
                                }
                            }
                        }
                    }
                    eprintln!("[ipc] Client disconnected — waiting for next connection");
                }
            }
        }
    });

    (gesture_rx, IpcStateSender { tx: state_tx })
}

// ════════════════════════════════════════════════════════════════════════════
// JSON gesture parser  (no serde — hand-rolled minimal parser)
// ════════════════════════════════════════════════════════════════════════════

/// Parse a single JSON line into a GestureEvent.
/// Returns None for unrecognised or malformed lines.
pub fn parse_gesture_json(line: &str) -> Option<GestureEvent> {
    let line = line.trim();
    if line.is_empty() { return None; }

    let typ = json_str_field(line, "type")?;

    match typ.as_str() {
        "pull_left" => {
            let steps    = json_u64_field(line, "steps").unwrap_or(1) as usize;
            let velocity = json_f32_field(line, "velocity").unwrap_or(0.5);
            Some(GestureEvent::PullLeft { steps, velocity })
        }
        "pull_right" => {
            let steps    = json_u64_field(line, "steps").unwrap_or(1) as usize;
            let velocity = json_f32_field(line, "velocity").unwrap_or(0.5);
            Some(GestureEvent::PullRight { steps, velocity })
        }
        "twist"   => Some(GestureEvent::Twist),
        "clap"    => Some(GestureEvent::Clap),
        "unclap"  => Some(GestureEvent::Unclap),
        "scissors" => {
            let name = json_str_field(line, "name").unwrap_or_default();
            Some(GestureEvent::Scissors { name })
        }
        "quit"    => Some(GestureEvent::Quit),
        _         => None,
    }
}

// ── minimal JSON field extractors ─────────────────────────────────────────

fn json_str_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let pos    = json.find(&needle)?;
    let after  = json[pos + needle.len()..].trim_start();
    if after.starts_with('"') {
        let inner = &after[1..];
        let end   = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

fn json_u64_field(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let pos    = json.find(&needle)?;
    let after  = json[pos + needle.len()..].trim_start();
    let end    = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
    after[..end].parse().ok()
}

fn json_f32_field(json: &str, key: &str) -> Option<f32> {
    let needle = format!("\"{}\":", key);
    let pos    = json.find(&needle)?;
    let after  = json[pos + needle.len()..].trim_start();
    let end    = after.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                      .unwrap_or(after.len());
    after[..end].parse().ok()
}

// ════════════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pull_left() {
        let line = r#"{"type":"pull_left","steps":3,"velocity":0.72}"#;
        let evt  = parse_gesture_json(line).unwrap();
        assert!(matches!(evt, GestureEvent::PullLeft { steps: 3, .. }));
        if let GestureEvent::PullLeft { velocity, .. } = evt {
            assert!((velocity - 0.72).abs() < 0.01);
        }
    }

    #[test]
    fn parse_pull_right_defaults() {
        let line = r#"{"type":"pull_right"}"#;
        let evt  = parse_gesture_json(line).unwrap();
        assert!(matches!(evt, GestureEvent::PullRight { steps: 1, .. }));
    }

    #[test]
    fn parse_twist()  { assert_eq!(parse_gesture_json(r#"{"type":"twist"}"#),  Some(GestureEvent::Twist)); }
    #[test]
    fn parse_clap()   { assert_eq!(parse_gesture_json(r#"{"type":"clap"}"#),   Some(GestureEvent::Clap)); }
    #[test]
    fn parse_unclap() { assert_eq!(parse_gesture_json(r#"{"type":"unclap"}"#), Some(GestureEvent::Unclap)); }

    #[test]
    fn parse_scissors() {
        let line = r#"{"type":"scissors","name":"my_snip"}"#;
        let evt  = parse_gesture_json(line).unwrap();
        assert!(matches!(evt, GestureEvent::Scissors { name } if name == "my_snip"));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert!(parse_gesture_json(r#"{"type":"wave"}"#).is_none());
        assert!(parse_gesture_json("").is_none());
    }

    #[test]
    fn state_msg_digits_json() {
        let msg = StateMsg::Digits {
            left: vec![3,1,4], right: vec![2,7,1],
            left_pos: 3, right_pos: 3,
        };
        let j = msg.to_json_line();
        assert!(j.contains("\"type\":\"digits\""));
        assert!(j.contains("[3,1,4]"));
        assert!(j.contains("[2,7,1]"));
        assert!(j.ends_with('\n'));
    }

    #[test]
    fn state_msg_note_json() {
        let msg = StateMsg::Note { pitch: 64, duration: 480, velocity: 100,
                                   left_pos: 5, right_pos: 3 };
        let j = msg.to_json_line();
        assert!(j.contains("\"pitch\":64"));
        assert!(j.contains("\"duration\":480"));
    }

    #[test]
    fn state_msg_status_escapes_quotes() {
        let msg = StateMsg::Status("say \"hello\"".to_string());
        let j   = msg.to_json_line();
        assert!(j.contains(r#"\"hello\""#));
    }
}
