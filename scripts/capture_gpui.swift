// Observe actual pixel changes from an isolated GPUI window via ScreenCaptureKit.
// Usage: swift capture_gpui.swift <pid> <events.csv> <captures.csv> <ready-file>
import AppKit
import AVFoundation
import CoreVideo
import Foundation
import ScreenCaptureKit

final class Collector: NSObject, SCStreamOutput {
    private var previous: Int?
    private(set) var changes: [(Int, Double)] = []
    private var frames = 0
    private let lock = NSLock()
    private let ready: String

    init(ready: String) { self.ready = ready }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, CMSampleBufferIsValid(sampleBuffer),
              let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        CVPixelBufferLockBaseAddress(pixels, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixels, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(pixels) else { return }
        let width = CVPixelBufferGetWidth(pixels)
        let height = CVPixelBufferGetHeight(pixels)
        let rowBytes = CVPixelBufferGetBytesPerRow(pixels)
        let raw = base.assumingMemoryBound(to: UInt8.self)
        guard width > 96, height > 140 else { return }
        let i = 100 * rowBytes + 20 * 4
        let b = raw[i], g = raw[i + 1], r = raw[i + 2]
        // Blue, red, green, yellow; robust to macOS display-profile conversion.
        let color: Int
        if b > 160 && r < 130 { color = 0 }
        else if r > 170 && g < 125 { color = 1 }
        else if g > 145 && r < 135 { color = 2 }
        else if r > 160 && g > 120 && b < 145 { color = 3 }
        else { return }
        lock.lock()
        defer { lock.unlock() }
        frames += 1
        if previous == nil {
            guard color == 0 else { return }
            previous = 0
            FileManager.default.createFile(atPath: ready, contents: Data())
        } else if let last = previous, last % 4 != color {
            var next = last + 1
            while next % 4 != color { next += 1 }
            previous = next
            changes.append((next, Date().timeIntervalSince1970 * 1000))
        }
    }

    func snapshot() -> ([(Int, Double)], Int) {
        lock.lock(); defer { lock.unlock() }
        return (changes, frames)
    }
}

@main struct Run {
    static func main() async throws {
        guard CommandLine.arguments.count == 5, let pid = Int32(CommandLine.arguments[1]) else {
            fputs("usage: capture_gpui <pid> <events.csv> <captures.csv> <ready-file>\n", stderr)
            exit(2)
        }
        let events = CommandLine.arguments[2], output = CommandLine.arguments[3], ready = CommandLine.arguments[4]
        _ = NSApplication.shared
        NSApplication.shared.setActivationPolicy(.accessory)
        var target: SCWindow?
        for _ in 0..<300 {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            target = content.windows.first { $0.owningApplication?.processID == pid && $0.frame.width > 300 }
            if target != nil { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        guard let window = target else { throw NSError(domain: "capture_gpui", code: 1, userInfo: [NSLocalizedDescriptionKey: "No on-screen GPUI window for PID \(pid)"]) }
        let config = SCStreamConfiguration()
        config.width = Int(window.frame.width)
        config.height = Int(window.frame.height)
        config.minimumFrameInterval = CMTime(value: 1, timescale: 60)
        config.queueDepth = 5
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.showsCursor = false
        let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: window), configuration: config, delegate: nil)
        let collector = Collector(ready: ready)
        try stream.addStreamOutput(collector, type: .screen, sampleHandlerQueue: DispatchQueue(label: "capture.gpui"))
        try await stream.startCapture()
        for _ in 0..<300 {
            if FileManager.default.fileExists(atPath: events) { break }
            try await Task.sleep(for: .milliseconds(100))
        }
        try await Task.sleep(for: .milliseconds(300))
        try? await stream.stopCapture() // The source app may have already closed its window.
        let (changes, frames) = collector.snapshot()
        let csv = "sequence,observed_epoch_ms\n" + changes.map { sequence, time in "\(sequence),\(String(format: "%.4f", time))\n" }.joined()
        try csv.write(toFile: output, atomically: true, encoding: .utf8)
        print("captured \(changes.count) marker changes from \(frames) ScreenCaptureKit frames (\(window.title ?? "GPUI window"))")
    }
}
