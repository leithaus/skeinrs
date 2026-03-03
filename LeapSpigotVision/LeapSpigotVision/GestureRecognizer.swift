// GestureRecognizer.swift
// ARKit hand tracking → GestureEvent, sent to SpigotEngine.

import ARKit
import RealityKit
import simd

// ── Thresholds ────────────────────────────────────────────────────────────────

private let CLAP_DIST_M:       Float = 0.10
private let UNCLAP_DIST_M:     Float = 0.22
private let PULL_VZ_MIN:       Float = 0.25   // m/s toward user
private let STEP_DIVISOR:      Float = 0.15
private let SCISSORS_ANGLE:    Float = 0.40   // radians spread between index/middle
private let TWIST_HEIGHT_DIFF: Float = 0.06
private let TWIST_FRAMES:      Int   = 8
private let SCISSORS_FRAMES:   Int   = 5
private let PULL_COOLDOWN:     Double = 0.08
private let SCISSORS_COOLDOWN: Double = 0.60

// ── GestureRecognizer ─────────────────────────────────────────────────────────

@MainActor
final class GestureRecognizer {

    weak var engine: SpigotEngine?

    private var wasClapped   = false
    private var twistCounter = 0
    private var scissorsL    = 0
    private var scissorsR    = 0
    private var lastPullL    = Date.distantPast
    private var lastPullR    = Date.distantPast
    private var lastScissors = Date.distantPast

    // Wrist velocity tracking
    private var prevWristPos:  [HandAnchor.Chirality: SIMD3<Float>] = [:]
    private var prevWristTime: [HandAnchor.Chirality: Date]         = [:]

    // Most recent anchors for two-hand checks
    private var latestLeft:  HandAnchor?
    private var latestRight: HandAnchor?

    // ── ARKit session ─────────────────────────────────────────────────────

    private let arSession    = ARKitSession()
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

    // ── Per-anchor processing ─────────────────────────────────────────────

    private func process(_ anchor: HandAnchor) async {
        guard anchor.isTracked, let skel = anchor.handSkeleton else { return }

        switch anchor.chirality {
        case .left:
            latestLeft = anchor
            handlePull(anchor: anchor, isLeft: true)
            handleScissors(skel: skel, isLeft: true)
        case .right:
            latestRight = anchor
            handlePull(anchor: anchor, isLeft: false)
            handleScissors(skel: skel, isLeft: false)
        @unknown default:
            break
        }

        if let l = latestLeft, let r = latestRight {
            handleClapAndTwist(left: l, right: r)
        }
    }

    // ── Pull ──────────────────────────────────────────────────────────────

    private func handlePull(anchor: HandAnchor, isLeft: Bool) {
        let vel      = palmVelocity(anchor: anchor)
        let lastPull = isLeft ? lastPullL : lastPullR
        guard vel.z > PULL_VZ_MIN,
              Date().timeIntervalSince(lastPull) > PULL_COOLDOWN else { return }

        if isLeft { lastPullL = Date() } else { lastPullR = Date() }
        let steps = max(1, Int(vel.z / STEP_DIVISOR))
        let v     = min(1.0, vel.z / 1.2)
        if isLeft { engine?.pullLeft(steps: steps, velocity: v) }
        else      { engine?.pullRight(steps: steps, velocity: v) }
    }

    // ── Scissors ──────────────────────────────────────────────────────────

    private func handleScissors(skel: HandSkeleton, isLeft: Bool) {
        if isScissors(skel) {
            if isLeft { scissorsL += 1 } else { scissorsR += 1 }
            let count = isLeft ? scissorsL : scissorsR
            if count >= SCISSORS_FRAMES,
               Date().timeIntervalSince(lastScissors) > SCISSORS_COOLDOWN {
                lastScissors = Date()
                if isLeft { scissorsL = 0 } else { scissorsR = 0 }
                Task { await promptSnipName() }
            }
        } else {
            if isLeft { scissorsL = 0 } else { scissorsR = 0 }
        }
    }

    // ── Clap + Twist ──────────────────────────────────────────────────────

    private func handleClapAndTwist(left: HandAnchor, right: HandAnchor) {
        guard left.isTracked && right.isTracked else { return }

        let lp   = anchorPosition(left)
        let rp   = anchorPosition(right)
        let dist = simd_distance(lp, rp)

        // Clap / unclap
        if !wasClapped && dist < CLAP_DIST_M {
            wasClapped = true
            engine?.clap()
        } else if wasClapped && dist > UNCLAP_DIST_M {
            wasClapped = false
            engine?.unclap()
        }

        // Twist
        let lOverR = lp.y > rp.y + TWIST_HEIGHT_DIFF
        let rOverL = rp.y > lp.y + TWIST_HEIGHT_DIFF
        if lOverR || rOverL {
            twistCounter += 1
            if twistCounter == TWIST_FRAMES { engine?.twist() }
        } else {
            twistCounter = 0
        }
    }

    // ── Scissors detection ────────────────────────────────────────────────

    /// True when index + middle are extended and spread, ring + pinky curled.
    private func isScissors(_ skel: HandSkeleton) -> Bool {
        guard fingerExtension(skel, tip: .indexFingerTip,  base: .indexFingerMetacarpal)  > 0.55,
              fingerExtension(skel, tip: .middleFingerTip, base: .middleFingerMetacarpal) > 0.55,
              fingerExtension(skel, tip: .ringFingerTip,   base: .ringFingerMetacarpal)   < 0.30,
              fingerExtension(skel, tip: .littleFingerTip, base: .littleFingerMetacarpal) < 0.30
        else { return false }

        // Spread angle between index and middle
        let indexDir = simd_normalize(
            jointPos(skel, .indexFingerTip) - jointPos(skel, .indexFingerKnuckle))
        let midDir   = simd_normalize(
            jointPos(skel, .middleFingerTip) - jointPos(skel, .middleFingerKnuckle))
        let angle = acos(simd_clamp(simd_dot(indexDir, midDir), -1, 1))
        return angle > SCISSORS_ANGLE
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    /// Normalised extension ratio (0 = fully curled, 1 = fully extended).
    private func fingerExtension(
        _ skel: HandSkeleton,
        tip:    HandSkeleton.JointName,
        base:   HandSkeleton.JointName
    ) -> Float {
        min(simd_distance(jointPos(skel, tip), jointPos(skel, base)) / 0.08, 1.0)
    }

    private func jointPos(_ skel: HandSkeleton, _ name: HandSkeleton.JointName) -> SIMD3<Float> {
        let t = skel.joint(name).anchorFromJointTransform
        return SIMD3<Float>(t.columns.3.x, t.columns.3.y, t.columns.3.z)
    }

    private func anchorPosition(_ anchor: HandAnchor) -> SIMD3<Float> {
        let t = anchor.originFromAnchorTransform
        return SIMD3<Float>(t.columns.3.x, t.columns.3.y, t.columns.3.z)
    }

    /// Palm velocity computed from successive wrist positions.
    private func palmVelocity(anchor: HandAnchor) -> SIMD3<Float> {
        let pos = anchorPosition(anchor)
        let now = Date()
        let key = anchor.chirality
        defer { prevWristPos[key] = pos; prevWristTime[key] = now }
        guard let prev = prevWristPos[key],
              let pt   = prevWristTime[key] else { return .zero }
        let dt = Float(now.timeIntervalSince(pt))
        guard dt > 0.001 else { return .zero }
        return (pos - prev) / dt
    }

    // ── Snippet name prompt ───────────────────────────────────────────────

    private func promptSnipName() async {
        await MainActor.run {
            NotificationCenter.default.post(
                name: .snippetNameRequested,
                object: nil)
        }
    }
}

// ── Notification name ─────────────────────────────────────────────────────────

extension Notification.Name {
    static let snippetNameRequested = Notification.Name("LeapSpigot.snippetNameRequested")
}
