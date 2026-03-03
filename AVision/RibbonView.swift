// RibbonView.swift
// RealityKit entities for the two spigot ribbons in 3D space.
//
// Layout:
//   Left ribbon  floats at world Y = +0.15 m (eye level, left of centre)
//   Right ribbon floats at world Y = -0.15 m (below, right of centre)
//   Patches are 6 cm wide × 4 cm tall × 1 cm deep boxes.
//   They extend along the Z axis (into the scene, away from the user).
//   The nearest patch is ~0.5 m in front of the user.
//
// When stitched (playing), gold thread cylinders span between the two ribbons.
// When snipped, the highlighted section pulses gold then flies to the tray.

import RealityKit
import SwiftUI
import simd

// ── Layout constants ──────────────────────────────────────────────────────────

private let PATCH_W:    Float = 0.06   // metres
private let PATCH_H:    Float = 0.04
private let PATCH_D:    Float = 0.01
private let PATCH_GAP:  Float = 0.005
private let PATCH_STEP: Float = PATCH_W + PATCH_GAP
private let NEAR_Z:     Float = -0.5   // distance in front of user (negative = forward in RealityKit)
private let Z_STEP:     Float = -0.07  // how far each successive patch is farther away
private let LEFT_Y:     Float =  0.15
private let RIGHT_Y:    Float = -0.15
private let RIBBON_X:   Float = -0.35  // left edge of ribbons

// ── RibbonRoot ────────────────────────────────────────────────────────────────

/// A root Entity that owns the two ribbon strips and the stitch threads.
/// Attach this to the ImmersiveSpace root.
class RibbonRoot: Entity {

    private var leftPatches:  [PatchEntity] = []
    private var rightPatches: [PatchEntity] = []
    private var stitchThreads: [Entity]     = []
    private var trayRoot: Entity

    required init() {
        trayRoot = Entity()
        super.init()
        trayRoot.position = [0.55, 0, -0.6]
        addChild(trayRoot)
    }

    // ── Update ribbons from engine state ──────────────────────────────────

    func update(digits: DigitState, base: UInt8 = 10) {
        synchronizePatches(
            patches:    &leftPatches,
            digits:     digits.left,
            worldY:     LEFT_Y,
            base:       base,
            labelColor: UIColor(red: 0.67, green: 0.87, blue: 1.0, alpha: 1)
        )
        synchronizePatches(
            patches:    &rightPatches,
            digits:     digits.right,
            worldY:     RIGHT_Y,
            base:       base,
            labelColor: UIColor(red: 1.0, green: 0.73, blue: 0.67, alpha: 1)
        )
    }

    private func synchronizePatches(
        patches:    inout [PatchEntity],
        digits:     [UInt8],
        worldY:     Float,
        base:       UInt8,
        labelColor: UIColor
    ) {
        // Add missing patches
        while patches.count < digits.count {
            let p = PatchEntity()
            addChild(p)
            patches.append(p)
        }
        // Update existing patches
        for (i, digit) in digits.enumerated() {
            let p   = patches[i]
            let z   = NEAR_Z + Float(i) * Z_STEP
            // Depth fade
            let fade = Float(i) / Float(max(digits.count, 1))
            p.configure(
                digit:     digit,
                base:      base,
                position:  [RIBBON_X, worldY, z],
                depthFade: fade,
                labelColor: labelColor
            )
            p.isEnabled = true
        }
        // Hide excess patches
        for i in digits.count..<patches.count {
            patches[i].isEnabled = false
        }
    }

    // ── Highlight the currently-playing patch ─────────────────────────────

    func highlightPatch(leftIndex: Int?) {
        for (i, p) in leftPatches.enumerated() {
            p.setHighlight(i == leftIndex)
        }
    }

    // ── Stitch threads ────────────────────────────────────────────────────

    func updateStitch(progress: Float) {
        // Remove old threads
        stitchThreads.forEach { $0.removeFromParent() }
        stitchThreads.removeAll()

        guard progress > 0 else { return }

        let count = min(leftPatches.count, rightPatches.count)
        let visible = Int(Float(count) * progress)

        for i in 0..<visible {
            guard i < leftPatches.count, i < rightPatches.count else { break }
            let lp = leftPatches[i].position
            let rp = rightPatches[i].position

            let thread = makeThread(from: lp, to: rp)
            addChild(thread)
            stitchThreads.append(thread)
        }
    }

    private func makeThread(from a: SIMD3<Float>, to b: SIMD3<Float>) -> Entity {
        let mid    = (a + b) * 0.5
        let len    = simd_distance(a, b)
        let dir    = simd_normalize(b - a)
        let up     = SIMD3<Float>(0, 0, 1)
        let axis   = simd_cross(up, dir)
        let angle  = acos(simd_dot(up, dir))
        let rot    = axis.x == 0 && axis.y == 0 && axis.z == 0
                   ? simd_quatf(angle: 0, axis: [0,0,1])
                   : simd_quatf(angle: angle, axis: simd_normalize(axis))

        let mesh    = MeshResource.generateCylinder(height: len, radius: 0.003)
        var mat     = UnlitMaterial()
        mat.color   = .init(tint: UIColor(red: 1, green: 0.84, blue: 0, alpha: 0.9))
        let entity  = ModelEntity(mesh: mesh, materials: [mat])
        entity.position    = mid
        entity.orientation = rot
        return entity
    }

    // ── Scissor highlight ─────────────────────────────────────────────────

    func highlightScissor(from startIdx: Int, count: Int, progress: Float) {
        let end = startIdx + Int(Float(count) * progress)
        for (i, p) in leftPatches.enumerated() {
            p.setScissorHighlight(i >= startIdx && i < end)
        }
        for (i, p) in rightPatches.enumerated() {
            p.setScissorHighlight(i >= startIdx && i < end)
        }
    }

    // ── Tray: deposit a snippet ───────────────────────────────────────────

    func depositSnippet(name: String, leftDigits: [UInt8], rightDigits: [UInt8], base: UInt8) {
        let entry = SnippetTrayEntry(name: name,
                                     leftDigits: leftDigits,
                                     rightDigits: rightDigits,
                                     base: base)
        let idx   = trayRoot.children.count
        entry.position = [0, Float(idx) * -0.08, 0]
        trayRoot.addChild(entry)

        // Animate flying in from left
        var anim = entry.position
        anim.x = -0.3
        entry.position = anim
        Task {
            try? await Task.sleep(for: .milliseconds(16))
            withAnimation { entry.position.x = 0 }
        }
    }
}

// ── PatchEntity ───────────────────────────────────────────────────────────────

class PatchEntity: Entity {

    private var box:   ModelEntity
    private var label: ModelEntity

    required init() {
        let mesh  = MeshResource.generateBox(width: PATCH_W, height: PATCH_H, depth: PATCH_D,
                                              cornerRadius: 0.004)
        var mat   = PhysicallyBasedMaterial()
        mat.baseColor = .init(tint: .gray)
        box   = ModelEntity(mesh: mesh, materials: [mat])

        // Tiny text label (digit)
        let textMesh  = MeshResource.generateText("0",
                            extrusionDepth: 0.002,
                            font: .systemFont(ofSize: 0.022))
        var textMat   = UnlitMaterial()
        textMat.color = .init(tint: .black)
        label = ModelEntity(mesh: textMesh, materials: [textMat])
        label.position = [-0.008, -0.010, PATCH_D / 2 + 0.001]

        super.init()
        addChild(box)
        addChild(label)
    }

    func configure(digit: UInt8, base: UInt8, position: SIMD3<Float>,
                   depthFade: Float, labelColor: UIColor) {
        self.position = position

        // Color from hue wheel
        let hue = CGFloat(digit) / CGFloat(max(base, 1))
        let color = UIColor(hue: hue, saturation: 0.82,
                            brightness: 0.92 * CGFloat(1 - depthFade * 0.7),
                            alpha: 1)
        var mat = PhysicallyBasedMaterial()
        mat.baseColor = .init(tint: color)
        mat.roughness = .init(floatLiteral: 0.4)
        box.model?.materials = [mat]

        // Update digit label
        let textMesh = MeshResource.generateText(
            "\(digit)",
            extrusionDepth: 0.002,
            font: .boldSystemFont(ofSize: 0.024))
        label.model?.mesh = textMesh

        // Scale down with distance
        let scale = 1.0 - depthFade * 0.5
        self.scale = [scale, scale, scale]
    }

    func setHighlight(_ on: Bool) {
        var mat = PhysicallyBasedMaterial()
        if on {
            mat.baseColor  = .init(tint: .white)
            mat.emissiveColor  = .init(color: UIColor(white: 0.8, alpha: 1))
            mat.emissiveIntensity = 0.6
        }
        // Reset handled by next configure() call
        if on { box.model?.materials = [mat] }
    }

    func setScissorHighlight(_ on: Bool) {
        if on {
            var mat = UnlitMaterial()
            mat.color = .init(tint: UIColor(red: 1, green: 1, blue: 0, alpha: 0.8))
            box.model?.materials = [mat]
        }
    }
}

// ── SnippetTrayEntry ──────────────────────────────────────────────────────────

class SnippetTrayEntry: Entity {

    required init(name: String, leftDigits: [UInt8], rightDigits: [UInt8], base: UInt8) {
        super.init()

        // Name label
        let nameMesh = MeshResource.generateText(name,
                            extrusionDepth: 0.002,
                            font: .boldSystemFont(ofSize: 0.018))
        var nameMat  = UnlitMaterial()
        nameMat.color = .init(tint: UIColor(red: 1, green: 0.84, blue: 0, alpha: 1))
        let nameEnt  = ModelEntity(mesh: nameMesh, materials: [nameMat])
        nameEnt.position = [0, 0.025, 0]
        addChild(nameEnt)

        // Mini ribbon strip
        let maxP = min(8, min(leftDigits.count, rightDigits.count))
        for i in 0..<maxP {
            let lPatch = makeMiniPatch(digit: leftDigits[i], base: base,
                                       y:  0.008, x: Float(i) * 0.022)
            let rPatch = makeMiniPatch(digit: rightDigits[i], base: base,
                                       y: -0.008, x: Float(i) * 0.022)
            addChild(lPatch)
            addChild(rPatch)
        }
    }

    required init() { super.init() }

    private func makeMiniPatch(digit: UInt8, base: UInt8, y: Float, x: Float) -> ModelEntity {
        let mesh  = MeshResource.generateBox(width: 0.018, height: 0.012, depth: 0.004)
        let hue   = CGFloat(digit) / CGFloat(max(base, 1))
        let color = UIColor(hue: hue, saturation: 0.8, brightness: 0.9, alpha: 1)
        var mat   = PhysicallyBasedMaterial()
        mat.baseColor = .init(tint: color)
        let e = ModelEntity(mesh: mesh, materials: [mat])
        e.position = [x, y, 0]
        return e
    }
}
