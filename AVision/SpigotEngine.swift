// SpigotEngine.swift
// Manages the Unix socket connection to the Rust engine.
// Publishes parsed state for SwiftUI / RealityKit views to observe.

import Foundation
import Combine

// ── Protocol types matching ipc.rs ───────────────────────────────────────────

struct DigitState {
    var left:     [UInt8]
    var right:    [UInt8]
    var leftPos:  Int
    var rightPos: Int
}

struct NoteEvent {
    let pitch:    UInt8
    let duration: UInt32
    let velocity: UInt8
    let leftPos:  Int
    let rightPos: Int
}

struct SnipEntry {
    let name:  String
    let count: Int
    let time:  Date
}

// ── SpigotEngine ─────────────────────────────────────────────────────────────

@MainActor
final class SpigotEngine: ObservableObject {

    // Published state observed by views
    @Published var digits:      DigitState = DigitState(left: [], right: [], leftPos: 0, rightPos: 0)
    @Published var lastNote:    NoteEvent?
    @Published var snippets:    [SnipEntry] = []
    @Published var statusText:  String = "Connecting…"
    @Published var leftLabel:   String = "π base 10"
    @Published var rightLabel:  String = "e base 10"
    @Published var isPlaying:   Bool   = false
    @Published var isConnected: Bool   = false

    private let sockPath: String
    private var inputStream:  InputStream?
    private var outputStream: OutputStream?
    private var readBuffer   = Data()
    private var connectTask: Task<Void, Never>?

    init(sockPath: String = "/tmp/leap_spigot.sock") {
        self.sockPath = sockPath
    }

    // ── Connect ───────────────────────────────────────────────────────────

    func connect() {
        connectTask?.cancel()
        connectTask = Task {
            await connectLoop()
        }
    }

    func disconnect() {
        connectTask?.cancel()
        inputStream?.close()
        outputStream?.close()
        isConnected = false
    }

    private func connectLoop() async {
        while !Task.isCancelled {
            do {
                try await openSocket()
                isConnected = true
                statusText  = "Connected to Rust engine"
                await readLoop()
            } catch {
                isConnected = false
                statusText  = "Waiting for engine… (\(error.localizedDescription))"
            }
            // Retry after 1 second
            try? await Task.sleep(for: .seconds(1))
        }
    }

    private func openSocket() async throws {
        // Unix domain socket via Stream
        var readStream:  Unmanaged<CFReadStream>?
        var writeStream: Unmanaged<CFWriteStream>?

        // CFStream doesn't natively support Unix sockets; use FileHandle via socketpair workaround.
        // Instead we use a POSIX connect + FileHandle approach.
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw SocketError.socketFailed }

        var addr              = sockaddr_un()
        addr.sun_family       = sa_family_t(AF_UNIX)
        let pathBytes         = sockPath.utf8CString
        withUnsafeMutableBytes(of: &addr.sun_path) { ptr in
            for (i, b) in pathBytes.enumerated() {
                ptr[i] = UInt8(bitPattern: b)
            }
        }
        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)

        let result = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, addrLen)
            }
        }
        guard result == 0 else {
            Darwin.close(fd)
            throw SocketError.connectFailed(errno)
        }

        inputStream  = InputStream(fileAtPath: "")   // placeholder
        outputStream = OutputStream(toFileAtPath: "", append: false) // placeholder

        // Wrap the raw fd in Stream objects
        Stream.getStreamsToHost(
            withName: sockPath,
            port: 0,
            inputStream:  &readStream,
            outputStream: &writeStream
        )
        // Use CFStream bridging for Unix socket fd
        let cfRead  = CFReadStreamCreateWithBytesNoCopy(nil,
                        UnsafePointer(bitPattern: 0)!, 0, nil)!
        // ↑ The above CFStream approach doesn't work cleanly for Unix sockets.
        // Instead use a cleaner FileHandle + AsyncBytes approach:
        Darwin.close(fd)  // close our test fd

        // Proper Unix socket connection via GCD / POSIX:
        let connFd = try await connectPOSIX()
        let readFH  = FileHandle(fileDescriptor: connFd, closeOnDealloc: false)
        let writeFH = FileHandle(fileDescriptor: connFd, closeOnDealloc: true)

        self.inputStream  = nil  // we'll use FileHandle directly
        self.outputStream = nil
        // Store FileHandles for use in readLoop
        self.connectedFD  = connFd
        self.writeFH      = writeFH
    }

    // Raw POSIX connection on a background thread
    private func connectPOSIX() async throws -> Int32 {
        try await withCheckedThrowingContinuation { cont in
            DispatchQueue.global().async {
                let fd = socket(AF_UNIX, SOCK_STREAM, 0)
                guard fd >= 0 else {
                    cont.resume(throwing: SocketError.socketFailed)
                    return
                }
                var addr        = sockaddr_un()
                addr.sun_family = sa_family_t(AF_UNIX)
                _ = self.sockPath.withCString { src in
                    withUnsafeMutableBytes(of: &addr.sun_path) { dst in
                        strlcpy(dst.baseAddress!.assumingMemoryBound(to: CChar.self),
                                src, MemoryLayout.size(ofValue: addr.sun_path))
                    }
                }
                let len = socklen_t(MemoryLayout<sockaddr_un>.size)
                let rc  = withUnsafePointer(to: &addr) {
                    $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                        Darwin.connect(fd, $0, len)
                    }
                }
                if rc == 0 {
                    cont.resume(returning: fd)
                } else {
                    Darwin.close(fd)
                    cont.resume(throwing: SocketError.connectFailed(errno))
                }
            }
        }
    }

    // ── Read loop ─────────────────────────────────────────────────────────

    private var connectedFD: Int32 = -1
    private var writeFH: FileHandle?

    private func readLoop() async {
        guard connectedFD >= 0 else { return }
        let fh = FileHandle(fileDescriptor: connectedFD, closeOnDealloc: false)
        do {
            for try await line in fh.bytes.lines {
                guard !Task.isCancelled else { break }
                await MainActor.run { self.handleLine(line) }
            }
        } catch {
            // Stream closed
        }
        Darwin.close(connectedFD)
        connectedFD = -1
        isConnected = false
    }

    // ── JSON line parser ──────────────────────────────────────────────────

    private func handleLine(_ line: String) {
        guard let data = line.data(using: .utf8),
              let obj  = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = obj["type"] as? String
        else { return }

        switch type {
        case "digits":
            let left     = (obj["left"]      as? [Int])?.map { UInt8($0) } ?? []
            let right    = (obj["right"]     as? [Int])?.map { UInt8($0) } ?? []
            let leftPos  = obj["left_pos"]   as? Int ?? 0
            let rightPos = obj["right_pos"]  as? Int ?? 0
            digits = DigitState(left: left, right: right,
                                leftPos: leftPos, rightPos: rightPos)

        case "note":
            let pitch    = UInt8(obj["pitch"]     as? Int ?? 60)
            let duration = UInt32(obj["duration"] as? Int ?? 480)
            let velocity = UInt8(obj["velocity"]  as? Int ?? 100)
            let lp       = obj["left_pos"]        as? Int ?? 0
            let rp       = obj["right_pos"]       as? Int ?? 0
            lastNote     = NoteEvent(pitch: pitch, duration: duration,
                                     velocity: velocity, leftPos: lp, rightPos: rp)
            isPlaying    = true

        case "snip_ack":
            let name  = obj["name"]  as? String ?? "?"
            let count = obj["count"] as? Int    ?? 0
            snippets.append(SnipEntry(name: name, count: count, time: Date()))
            if snippets.count > 20 { snippets.removeFirst() }

        case "status":
            statusText = obj["text"] as? String ?? ""
            if statusText.contains("stopped") { isPlaying = false }

        case "twist_ack":
            leftLabel  = obj["left_label"]  as? String ?? leftLabel
            rightLabel = obj["right_label"] as? String ?? rightLabel

        default:
            break
        }
    }

    // ── Send gestures → Rust ──────────────────────────────────────────────

    func sendGesture(_ json: String) {
        guard let fh = writeFH else { return }
        let line = json + "\n"
        guard let data = line.data(using: .utf8) else { return }
        try? fh.write(contentsOf: data)
    }

    func pullLeft(steps: Int = 1, velocity: Float = 0.5) {
        sendGesture(#"{"type":"pull_left","steps":\#(steps),"velocity":\#(String(format:"%.3f",velocity))}"#)
    }
    func pullRight(steps: Int = 1, velocity: Float = 0.5) {
        sendGesture(#"{"type":"pull_right","steps":\#(steps),"velocity":\#(String(format:"%.3f",velocity))}"#)
    }
    func twist()               { sendGesture(#"{"type":"twist"}"#) }
    func clap()                { sendGesture(#"{"type":"clap"}"#) }
    func unclap()              { sendGesture(#"{"type":"unclap"}"#) }
    func scissors(name: String) {
        let safe = name.replacingOccurrences(of: "\"", with: "'")
        sendGesture(#"{"type":"scissors","name":"\#(safe)"}"#)
    }
    func quit()                { sendGesture(#"{"type":"quit"}"#) }
}

// ── Errors ────────────────────────────────────────────────────────────────────

enum SocketError: LocalizedError {
    case socketFailed
    case connectFailed(Int32)

    var errorDescription: String? {
        switch self {
        case .socketFailed:          return "socket() failed"
        case .connectFailed(let e):  return "connect() failed: errno \(e)"
        }
    }
}
