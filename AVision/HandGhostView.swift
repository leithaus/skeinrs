// HandGhostView.swift
// Wireframe hand skeletons that mirror the user's actual hand poses
// and animate to show the recognised gesture state.
//
// Uses HandAnchor joint transforms to position ModelEntity cylinders
// for each bone segment.

import RealityKit
import ARKit
import simd

// ── HandGhostRoot ─────────────────────────────────────────────────────────────

/// Container for both hand ghost entities.
/// Attach to the ImmersiveSpace root.
class HandGhostRoot: Entity {

    let leftGhost  = HandGhostEntity(chirality: .left)
    let rightGhost = HandGhostEntity(chirality: .right)

    required init() {
        super.init()
        addChild(leftGhost)
        addChild(rightGhost)
    }
}

// ── HandGhostEntity ───────────────────────────────────────────────────────────

/// A single-hand wireframe ghost rendered as thin cylinder segments
/// connecting each pair of adjacent hand joints.
class HandGhostEntity: Entity {

    let chirality: HandAnchor.Chirality

    // One ModelEntity per bone segment
    private var bones: [String: ModelEntity] = [:]

    // Gesture label floating above the hand
    private let gestureLabel: ModelEntity

    // Tint colour: blue for left, warm for right
    private var tintColor: UIColor {
        chirality == .left
          ? UIColor(red: 0.67, green: 0.87, blue: 1.0,  alpha: 0.85)
          : UIColor(red: 1.0,  green: 0.73, blue: 0.67, alpha: 0.85)
    }

    required init(chirality: HandAnchor.Chirality) {
        self.chirality = chirality
        let labelMesh = MeshResource.generateText("",
                            extrusionDepth: 0.001,
                            font: .systemFont(ofSize: 0.014))
        var mat   = UnlitMaterial()
        mat.color = .init(tint: UIColor.white)
        gestureLabel = ModelEntity(mesh: labelMesh, materials: [mat])
        super.init()
        addChild(gestureLabel)
    }

    required init() {
        fatalError("use init(chirality:)")
    }

    // ── Update from live ARKit anchor ─────────────────────────────────────

    func update(anchor: HandAnchor, gestureName: String) {
        guard anchor.isTracked, let skel = anchor.handSkeleton else {
            isEnabled = false
            return
        }
        isEnabled     = true
        // Position the whole ghost entity at the anchor's origin
        let originT   = anchor.originFromAnchorTransform
        position      = SIMD3<Float>(originT.columns.3.x,
                                      originT.columns.3.y,
                                      originT.columns.3.z)
        orientation   = simd_quatf(originT)

        // Render bone segments
        let segments = HandSkeleton.boneSegments
        for seg in segments {
            let aPos = jointLocalPos(skel, seg.joint0)
            let bPos = jointLocalPos(skel, seg.joint1)
            updateBone(id: seg.id, from: aPos, to: bPos)
        }

        // Update gesture label
        let lmesh = MeshResource.generateText(gestureName,
                        extrusionDepth: 0.001,
                        font: .systemFont(ofSize: 0.014))
        gestureLabel.model?.mesh = lmesh
        gestureLabel.position = [0, 0.12, 0]
    }

    // ── Bone rendering ────────────────────────────────────────────────────

    private func updateBone(id: String, from a: SIMD3<Float>, to b: SIMD3<Float>) {
        let len = simd_distance(a, b)
        guard len > 0.001 else { return }

        if bones[id] == nil {
            let bone = makeBone()
            addChild(bone)
            bones[id] = bone
        }
        let bone = bones[id]!
        bone.position    = (a + b) * 0.5
        bone.scale       = [1, len / 0.01, 1]   // scale Y to match length (base mesh is 1 cm)
        bone.orientation = rotationFrom(up: [0,1,0], to: simd_normalize(b - a))
    }

    private func makeBone() -> ModelEntity {
        let mesh  = MeshResource.generateCylinder(height: 0.01, radius: 0.003)
        var mat   = UnlitMaterial()
        mat.color = .init(tint: tintColor)
        return ModelEntity(mesh: mesh, materials: [mat])
    }

    private func jointLocalPos(_ skel: HandSkeleton, _ name: HandSkeleton.JointName) -> SIMD3<Float> {
        let t = skel.joint(name).anchorFromJointTransform
        return SIMD3<Float>(t.columns.3.x, t.columns.3.y, t.columns.3.z)
    }

    private func rotationFrom(up: SIMD3<Float>, to dir: SIMD3<Float>) -> simd_quatf {
        let axis  = simd_cross(up, dir)
        let angle = acos(simd_clamp(simd_dot(up, dir), -1, 1))
        let len   = simd_length(axis)
        guard len > 1e-6 else { return simd_quatf(angle: 0, axis: [0,1,0]) }
        return simd_quatf(angle: angle, axis: axis / len)
    }
}

// ── HandSkeleton bone segment definitions ─────────────────────────────────────

struct BoneSegment {
    let id:     String
    let joint0: HandSkeleton.JointName
    let joint1: HandSkeleton.JointName
}

extension HandSkeleton {
    static let boneSegments: [BoneSegment] = [
        // Thumb
        BoneSegment(id: "thumb_0", joint0: .wrist,                  joint1: .thumbKnuckle),
        BoneSegment(id: "thumb_1", joint0: .thumbKnuckle,           joint1: .thumbIntermediateBase),
        BoneSegment(id: "thumb_2", joint0: .thumbIntermediateBase,  joint1: .thumbIntermediateTip),
        BoneSegment(id: "thumb_3", joint0: .thumbIntermediateTip,   joint1: .thumbTip),
        // Index
        BoneSegment(id: "idx_0",   joint0: .wrist,                  joint1: .indexFingerMetacarpal),
        BoneSegment(id: "idx_1",   joint0: .indexFingerMetacarpal,  joint1: .indexFingerKnuckle),
        BoneSegment(id: "idx_2",   joint0: .indexFingerKnuckle,     joint1: .indexFingerIntermediateBase),
        BoneSegment(id: "idx_3",   joint0: .indexFingerIntermediateBase, joint1: .indexFingerIntermediateTip),
        BoneSegment(id: "idx_4",   joint0: .indexFingerIntermediateTip,  joint1: .indexFingerTip),
        // Middle
        BoneSegment(id: "mid_0",   joint0: .wrist,                  joint1: .middleFingerMetacarpal),
        BoneSegment(id: "mid_1",   joint0: .middleFingerMetacarpal, joint1: .middleFingerKnuckle),
        BoneSegment(id: "mid_2",   joint0: .middleFingerKnuckle,    joint1: .middleFingerIntermediateBase),
        BoneSegment(id: "mid_3",   joint0: .middleFingerIntermediateBase, joint1: .middleFingerIntermediateTip),
        BoneSegment(id: "mid_4",   joint0: .middleFingerIntermediateTip,  joint1: .middleFingerTip),
        // Ring
        BoneSegment(id: "ring_0",  joint0: .wrist,                  joint1: .ringFingerMetacarpal),
        BoneSegment(id: "ring_1",  joint0: .ringFingerMetacarpal,   joint1: .ringFingerKnuckle),
        BoneSegment(id: "ring_2",  joint0: .ringFingerKnuckle,      joint1: .ringFingerIntermediateBase),
        BoneSegment(id: "ring_3",  joint0: .ringFingerIntermediateBase, joint1: .ringFingerIntermediateTip),
        BoneSegment(id: "ring_4",  joint0: .ringFingerIntermediateTip,  joint1: .ringFingerTip),
        // Pinky
        BoneSegment(id: "pink_0",  joint0: .wrist,                  joint1: .littleFingerMetacarpal),
        BoneSegment(id: "pink_1",  joint0: .littleFingerMetacarpal, joint1: .littleFingerKnuckle),
        BoneSegment(id: "pink_2",  joint0: .littleFingerKnuckle,    joint1: .littleFingerIntermediateBase),
        BoneSegment(id: "pink_3",  joint0: .littleFingerIntermediateBase, joint1: .littleFingerIntermediateTip),
        BoneSegment(id: "pink_4",  joint0: .littleFingerIntermediateTip,  joint1: .littleFingerTip),
        // Palm knuckle connections
        BoneSegment(id: "palm_0",  joint0: .indexFingerMetacarpal,  joint1: .middleFingerMetacarpal),
        BoneSegment(id: "palm_1",  joint0: .middleFingerMetacarpal, joint1: .ringFingerMetacarpal),
        BoneSegment(id: "palm_2",  joint0: .ringFingerMetacarpal,   joint1: .littleFingerMetacarpal),
    ]
}
