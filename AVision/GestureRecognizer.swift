// GestureRecognizer.swift
// ARKit hand tracking → GestureEvent, sent to SpigotEngine.
//
// Gesture detection mirrors the Rust LeapGestureSource logic,
// adapted to the ARKit HandAnchor / HandSkeleton API.
//
// Requires: visionOS 1.0+, ARKit hand tracking entitlement.

import ARKit
import RealityKit
import simd

// ── Thresholds (tuned for AVP hand tracking) ──────────────────────────────────

private let CLAP_DIST_M:      Float = 0.10   // 10 cm — hands this close = clap
private let UNCLAP_DIST_M:    Float = 0.22   // 22 cm — hands this far = unclap
private let PULL_VZ_MIN:      Float = 0.25   // m/s toward user
private let STEP_DIVISOR:     Float = 0.15   // m/s per step
private let SCISSORS_ANGLE:   Float = 0.40   // radians spread between index/middle
private let TWIST_HEIGHT_DIFF:Float = 0.06   // 6 cm vertical separation for twist
private let TWIST_FRAMES:     Int   = 8
private let SCISSORS_FRAMES:  Int   = 5
private let PULL_COOLDOWN:    Double = 0.08  // seconds
private let SCISSORS_COOLDOWN:Double = 0.60

// ── GestureRecognizer ─────────────────────────────────────────────────────────

@MainActor
final class GestureRecognizer {

    weak var engine: SpigotEngine?

    // State
    private var wasClappped      = false
    private var twistCounter     = 0
    private var scissorsL        = 0
    private var scissorsR        = 0
    private var lastPullL        = Date.distantPast
    private var lastPullR        = Date.distantPast
    private var lastScissors     = Date.distantPast

    // ── ARKit session ─────────────────────────────────────────────────────

    private let arSession = ARKitSession()
    private let handProvider = HandTrackingProvider()

    func start() async {
        guard HandTrackingProvider.isSupported else {
            print("[gesture] Hand tracking not supported on this device")
            return
        }
        do {
            try await arSession.run([handProvider])
            for await update in handProvider.anchorUpdates {
                await process(update.anchor)
            }
        } catch {
            print("[gesture] ARKit error: \(error)")
        }
    }

    // ── Per-frame hand processing ─────────────────────────────────────────

    private func process(_ anchor: HandAnchor) async {
        guard anchor.isTracked,
              let skel = anchor.handSkeleton else { return }

        let chirality = anchor.chirality  // .left or .right
        let palmPose  = anchor.originFromAnchorTransform
        let palmPos   = SIMD3<Float>(palmPose.columns.3.x,
                                     palmPose.columns.3.y,
                                     palmPose.columns.3.z)

        // Estimate palm velocity from successive positions (simple finite diff)
        // In a real app you'd track previous position; here we read velocity
        // from the joint angular velocity as a proxy.
        // AVP doesn't expose linear velocity directly so we derive it from
        // the rate of change of the wrist joint transform.
        let vel = palmVelocity(anchor: anchor)

        switch chirality {
        case .left:
            // Pull left?
            if vel.z > PULL_VZ_MIN && Date().timeIntervalSince(lastPullL) > PULL_COOLDOWN {
                lastPullL = Date()
                let steps = max(1, Int(vel.z / STEP_DIVISOR))
                let v     = min(1.0, vel.z / 1.2)
                engine?.pullLeft(steps: steps, velocity: v)
            }
            // Scissors?
            if isScissors(skel) {
                scissorsL += 1
                if scissorsL >= SCISSORS_FRAMES
                    && Date().timeIntervalSince(lastScissors) > SCISSORS_COOLDOWN {
                    lastScissors = Date()
                    await promptSnipName()
                }
            } else {
                scissorsL = 0
            }

        case .right:
            if vel.z > PULL_VZ_MIN && Date().timeIntervalSince(lastPullR) > PULL_COOLDOWN {
                lastPullR = Date()
                let steps = max(1, Int(vel.z / STEP_DIVISOR))
                let v     = min(1.0, vel.z / 1.2)
                engine?.pullRight(steps: steps, velocity: v)
            }
            if isScissors(skel) {
                scissorsR += 1
                if scissorsR >= SCISSORS_FRAMES
                    && Date().timeIntervalSince(lastScissors) > SCISSORS_COOLDOWN {
                    lastScissors = Date()
                    await promptSnipName()
                }
            } else {
                scissorsR = 0
            }

        @unknown default:
            break
        }

        // Clap / Unclap / Twist require both hands — handled in processBothHands()
    }

    // Called with both hands every frame when both are tracked
    func processBothHands(left: HandAnchor, right: HandAnchor) {
        guard left.isTracked && right.isTracked else { return }

        let lp = position(left)
        let rp = position(right)
        let dist = simd_distance(lp, rp)

        // Clap
        if !wasClappped && dist < CLAP_DIST_M {
            wasClappped = true
            engine?.clap()
        } else if wasClappped && dist > UNCLAP_DIST_M {
            wasClappped = false
            engine?.unclap()
        }

        // Twist — left palm above right or vice versa, sustained
        let lOverR = lp.y > rp.y + TWIST_HEIGHT_DIFF
        let rOverL = rp.y > lp.y + TWIST_HEIGHT_DIFF
        if lOverR || rOverL {
            twistCounter += 1
            if twistCounter == TWIST_FRAMES {
                engine?.twist()
            }
        } else {
            twistCounter = 0
        }
    }

    // ── Gesture helpers ───────────────────────────────────────────────────

    /// Is the hand making a scissors gesture?
    /// Index + middle extended with spread > threshold; ring + pinky curled.
    private func isScissors(_ skel: HandSkeleton) -> Bool {
        let indexExt  = fingerExtension(skel, finger: .indexFinger)  > 0.55
        let middleExt = fingerExtension(skel, finger: .middleFinger) > 0.55
        let ringCurl  = fingerExtension(skel, finger: .ringFinger)   < 0.30
        let pinkyCurl = fingerExtension(skel, finger: .littleFinger) < 0.30
        guard indexExt && middleExt && ringCurl && pinkyCurl else { return false }

        // Check spread angle between index and middle tip directions
        let indexTip  = jointPos(skel, .indexFingerTip)
        let indexBase = jointPos(skel, .indexFingerKnuckle)
        let midTip    = jointPos(skel, .middleFingerTip)
        let midBase   = jointPos(skel, .middleFingerKnuckle)

        let id = simd_normalize(indexTip - indexBase)
        let md = simd_normalize(midTip   - midBase)
        let angle = acos(simd_clamp(simd_dot(id, md), -1, 1))
        return angle > SCISSORS_ANGLE
    }

    /// Ratio of finger extension 0 (curled) → 1 (straight).
    private func fingerExtension(_ skel: HandSkeleton, finger: HandSkeleton.JointName) -> Float {
        // Use tip–metacarpal distance normalised to ~80 mm max
        let tip  = jointPos(skel, tipJoint(for: finger))
        let base = jointPos(skel, metacarpalJoint(for: finger))
        return min(simd_distance(tip, base) / 0.08, 1.0)
    }

    private func jointPos(_ skel: HandSkeleton, _ name: HandSkeleton.JointName) -> SIMD3<Float> {
        let t = skel.joint(name).anchorFromJointTransform
        return SIMD3<Float>(t.columns.3.x, t.columns.3.y, t.columns.3.z)
    }

    private func position(_ anchor: HandAnchor) -> SIMD3<Float> {
        let t = anchor.originFromAnchorTransform
        return SIMD3<Float>(t.columns.3.x, t.columns.3.y, t.columns.3.z)
    }

    /// Approximate palm velocity from wrist transform change.
    /// In production you'd track previous transform over time.
    private func palmVelocity(anchor: HandAnchor) -> SIMD3<Float> {
        // AVP does not expose velocity directly in HandAnchor.
        // Real implementation: store previous wrist position + timestamp,
        // compute (pos_now - pos_prev) / dt each frame.
        // Placeholder returns zero; replace with your tracking logic.
        return .zero
    }

    // ── Snippet name prompt ───────────────────────────────────────────────
    // In visionOS, show a sheet / text field rather than stdin.
    // This is called on the main actor so UI is safe.

    private func promptSnipName() async {
        // Post a notification that the UI observes to show the text field.
        await MainActor.run {
            NotificationCenter.default.post(
                name: .snippetNameRequested,
                object: nil
            )
        }
    }

    // ── Joint name helpers ────────────────────────────────────────────────

    private func tipJoint(for finger: HandSkeleton.JointName) -> HandSkeleton.JointName {
        switch finger {
        case .indexFinger:  return .indexFingerTip
        case .middleFinger: return .middleFingerTip
        case .ringFinger:   return .ringFingerTip
        case .littleFinger: return .littleFingerTip
        default:            return .indexFingerTip
        }
    }

    private func metacarpalJoint(for finger: HandSkeleton.JointName) -> HandSkeleton.JointName {
        switch finger {
        case .indexFinger:  return .indexFingerMetacarpal
        case .middleFinger: return .middleFingerMetacarpal
        case .ringFinger:   return .ringFingerMetacarpal
        case .littleFinger: return .littleFingerMetacarpal
        default:            return .indexFingerMetacarpal
        }
    }
}

// ── Notification name ─────────────────────────────────────────────────────────

extension Notification.Name {
    static let snippetNameRequested = Notification.Name("LeapSpigot.snippetNameRequested")
}
