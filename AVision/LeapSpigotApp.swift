// LeapSpigotApp.swift
// visionOS app entry point.
//
// Build requirements:
//   Xcode 15.2+, visionOS SDK 1.0+
//   Entitlements: com.apple.developer.arkit.hand-tracking
//
// Run the Rust engine first:
//   cd leap_spigot && cargo run -- --quick --ipc
// Then launch this app in the visionOS simulator or on device.

import SwiftUI
import RealityKit
import ARKit

@main
struct LeapSpigotApp: App {

    @StateObject private var engine = SpigotEngine()

    var body: some Scene {

        // ── Main window: status + snippet tray ───────────────────────────
        WindowGroup {
            ContentView()
                .environmentObject(engine)
        }
        .windowStyle(.plain)
        .defaultSize(width: 480, height: 320)

        // ── Immersive space: 3D ribbons + hand ghosts ─────────────────────
        ImmersiveSpace(id: "RibbonSpace") {
            RibbonImmersiveView()
                .environmentObject(engine)
        }
        .immersionStyle(selection: .constant(.mixed), in: .mixed)
    }
}

// ── ContentView — 2D overlay panel ────────────────────────────────────────────

struct ContentView: View {

    @EnvironmentObject var engine: SpigotEngine
    @Environment(\.openImmersiveSpace)  var openImmersiveSpace
    @Environment(\.dismissImmersiveSpace) var dismissImmersiveSpace
    @State private var isImmersive = false
    @State private var showSnipSheet = false
    @State private var snippetName  = ""

    var body: some View {
        VStack(spacing: 16) {

            // ── Header ────────────────────────────────────────────────────
            HStack {
                Circle()
                    .fill(engine.isConnected ? Color.green : Color.red)
                    .frame(width: 10, height: 10)
                Text(engine.isConnected ? "Engine connected" : "Waiting for engine…")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text("Leap Spigot")
                    .font(.headline)
            }
            .padding(.horizontal)

            Divider()

            // ── Ribbon labels + stream info ───────────────────────────────
            HStack(spacing: 32) {
                streamInfo(label: engine.leftLabel,
                           digits: engine.digits.left,
                           pos:    engine.digits.leftPos,
                           color:  Color(red: 0.67, green: 0.87, blue: 1.0))
                Spacer()
                if engine.isPlaying {
                    Image(systemName: "music.note")
                        .foregroundStyle(.yellow)
                        .symbolEffect(.pulse)
                }
                Spacer()
                streamInfo(label: engine.rightLabel,
                           digits: engine.digits.right,
                           pos:    engine.digits.rightPos,
                           color:  Color(red: 1.0, green: 0.73, blue: 0.67))
            }
            .padding(.horizontal)

            // ── Status bar ────────────────────────────────────────────────
            Text(engine.statusText)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal)

            Divider()

            // ── Snippet tray ──────────────────────────────────────────────
            if !engine.snippets.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(engine.snippets.indices, id: \.self) { i in
                            let s = engine.snippets[i]
                            SnippetChip(entry: s)
                        }
                    }
                    .padding(.horizontal)
                }
                .frame(height: 52)
            }

            Divider()

            // ── Controls ──────────────────────────────────────────────────
            HStack(spacing: 12) {
                Button(isImmersive ? "Exit 3D" : "Enter 3D") {
                    Task {
                        if isImmersive {
                            await dismissImmersiveSpace()
                        } else {
                            await openImmersiveSpace(id: "RibbonSpace")
                        }
                        isImmersive.toggle()
                    }
                }
                .buttonStyle(.borderedProminent)

                Button("Clap ▶") { engine.clap() }
                    .disabled(!engine.isConnected || engine.isPlaying)
                Button("■ Stop")  { engine.unclap() }
                    .disabled(!engine.isPlaying)
                Button("Twist ⇄") { engine.twist() }
                Button("✂ Snip")  { showSnipSheet = true }
            }
            .padding(.horizontal)
        }
        .padding(.vertical)
        .onAppear {
            engine.connect()
        }
        .onDisappear {
            engine.disconnect()
        }
        .onReceive(NotificationCenter.default.publisher(for: .snippetNameRequested)) { _ in
            showSnipSheet = true
        }
        .sheet(isPresented: $showSnipSheet) {
            SnipNameSheet(name: $snippetName) { name in
                engine.scissors(name: name)
                snippetName = ""
            }
        }
    }

    @ViewBuilder
    private func streamInfo(label: String, digits: [UInt8], pos: Int, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label)
                .font(.caption2.bold())
                .foregroundStyle(color)
            HStack(spacing: 2) {
                ForEach(digits.prefix(12).indices, id: \.self) { i in
                    digitSquare(digits[i], color: color)
                }
            }
            Text("pos \(pos)")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
    }

    @ViewBuilder
    private func digitSquare(_ d: UInt8, color: Color) -> some View {
        Text("\(d)")
            .font(.system(size: 9, design: .monospaced))
            .frame(width: 16, height: 16)
            .background(color.opacity(0.25))
            .cornerRadius(2)
    }
}

// ── SnippetChip ───────────────────────────────────────────────────────────────

struct SnippetChip: View {
    let entry: SnipEntry
    var body: some View {
        VStack(spacing: 2) {
            Text(entry.name)
                .font(.caption2.bold())
                .foregroundStyle(.yellow)
            Text("\(entry.count) pairs")
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(.ultraThinMaterial)
        .cornerRadius(6)
    }
}

// ── SnipNameSheet ─────────────────────────────────────────────────────────────

struct SnipNameSheet: View {
    @Binding var name: String
    let onConfirm: (String) -> Void
    @Environment(\.dismiss) var dismiss

    var body: some View {
        VStack(spacing: 20) {
            Text("Name this snippet")
                .font(.headline)
            TextField("snippet name", text: $name)
                .textFieldStyle(.roundedBorder)
                .frame(width: 260)
            HStack {
                Button("Cancel") { dismiss() }
                    .buttonStyle(.bordered)
                Button("Save") {
                    let n = name.isEmpty ? "snip_\(Int(Date().timeIntervalSince1970))" : name
                    onConfirm(n)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .disabled(false)
            }
        }
        .padding(32)
    }
}

// ── RibbonImmersiveView ───────────────────────────────────────────────────────

struct RibbonImmersiveView: View {

    @EnvironmentObject var engine: SpigotEngine

    // Owned RealityKit entities
    @State private var ribbonRoot  = RibbonRoot()
    @State private var handGhosts  = HandGhostRoot()
    @State private var gestureRecog = GestureRecognizer()

    var body: some View {
        RealityView { content in
            content.add(ribbonRoot)
            content.add(handGhosts)
            gestureRecog.engine = engine
        } update: { _ in
            // Sync ribbons from engine state
            ribbonRoot.update(digits: engine.digits)
            if let note = engine.lastNote {
                ribbonRoot.highlightPatch(leftIndex: nil) // index resolved in engine
            }
        }
        .task {
            await gestureRecog.start()
        }
        .onChange(of: engine.digits) { _, _ in
            ribbonRoot.update(digits: engine.digits)
        }
        .onChange(of: engine.snippets) { old, new in
            guard let latest = new.last, new.count > old.count else { return }
            let ld = engine.digits.left
            let rd = engine.digits.right
            ribbonRoot.depositSnippet(name: latest.name,
                                      leftDigits: ld, rightDigits: rd, base: 10)
        }
    }
}
