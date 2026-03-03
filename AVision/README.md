# LeapSpigotVision — Apple Vision Pro Frontend

visionOS app that connects to the `leap_spigot` Rust engine over a Unix
domain socket and renders the spigot streams as 3D ribbons in mixed reality.

## Architecture

```
┌─────────────────────────────────┐     Unix socket      ┌────────────────────────────┐
│   LeapSpigotVision (visionOS)   │  /tmp/leap_spigot.sock│   leap_spigot --ipc (macOS)│
│                                 │ ◄──── state JSON ──── │                            │
│  GestureRecognizer (ARKit)      │ ──── gesture JSON ──► │  spigot_stream (π, e, …)   │
│  RibbonView (RealityKit)        │                       │  dual_spigot               │
│  HandGhostView (RealityKit)     │                       │  spigot_midi → MIDI out    │
│  SpigotEngine (socket client)   │                       │  ipc::IpcGestureSource     │
│  LeapSpigotApp (SwiftUI)        │                       │                            │
└─────────────────────────────────┘                       └────────────────────────────┘
```

## Prerequisites

- Mac with Apple Silicon (M1/M2/M3)
- Xcode 15.2 or later
- visionOS SDK 1.0+
- Apple Vision Pro device **or** visionOS Simulator (Xcode → Window → Devices & Simulators)
- Rust toolchain (for the engine)

## Running

### 1. Start the Rust engine

```bash
cd leap_spigot
cargo run --release -- --quick --ipc
# Prints: [ipc] Listening on /tmp/leap_spigot.sock
```

### 2. Open the Xcode project

```bash
open LeapSpigotVision/LeapSpigotVision.xcodeproj
```

Select the `LeapSpigotVision` scheme, choose the visionOS Simulator or your
device, then **⌘R** to build and run.

The status indicator in the 2D panel turns green once the socket connection
is established.

### 3. Enter the immersive space

Tap **Enter 3D** in the 2D panel.  Two coloured ribbons of digit-patches
float in your space, receding into the distance.  Your hand skeletons are
mirrored as wireframe ghosts.

## Gesture reference

| Gesture | Action |
|---------|--------|
| Pull left hand toward you | Advance left (duration) stream |
| Pull right hand toward you | Advance right (pitch) stream |
| Faster pull | Faster advance |
| Left hand over right (or vice versa) | Twist — swap streams |
| Bring both hands together (clap) | Start MIDI playback; ribbons stitch |
| Pull hands apart (unclap) | Stop MIDI; ribbons separate |
| Index + middle spread, others curled (scissors) | Snip — name sheet appears |

## File overview

| File | Purpose |
|------|---------|
| `LeapSpigotApp.swift` | App entry, `ContentView` (2D panel), `RibbonImmersiveView` |
| `SpigotEngine.swift` | Socket client, JSON codec, `@Published` state |
| `GestureRecognizer.swift` | ARKit hand tracking → gesture events → `SpigotEngine` |
| `RibbonView.swift` | RealityKit ribbon entities, patch boxes, stitch threads, tray |
| `HandGhostView.swift` | Wireframe hand skeletons mirroring live joint positions |
| `Info.plist` | Bundle metadata, `NSHandsTrackingUsageDescription` |
| `LeapSpigotVision.entitlements` | Hand tracking + sandbox entitlements |

## Known limitations / next steps

- **Palm velocity** is currently a stub returning zero in `GestureRecognizer`.
  Implement it by tracking the wrist position across successive `HandAnchor`
  updates and computing `Δposition / Δtime`.

- The visionOS Simulator does **not** support hand tracking.  Use the
  **2D panel buttons** (Clap, Twist, ✂ Snip) while testing in the simulator;
  physical gestures only work on device.

- **MIDI output**: `spigot_midi` uses `midir` which on macOS routes to
  CoreMIDI.  The notes play on the Mac's audio even while the visionOS app
  runs on device, because the engine runs on the Mac.  For fully on-device
  audio, the MIDI generation would need to move into the Swift app using
  `AVAudioEngine` / `MIDIKit`.

- **Snippet persistence**: snippets are in-memory only.  Add `Codable`
  serialisation to `SnipEntry` and write to `AppStorage` or a file to persist
  across sessions.
