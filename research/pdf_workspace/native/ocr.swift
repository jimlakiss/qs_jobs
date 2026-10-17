// Local OCR helper: bounded RGBA crop on stdin, JSON text/confidence on stdout.
import Foundation
import Darwin
import Vision
import CoreGraphics

// Exit promptly if cancellation kills the enclosing capture worker.
let parentPID = getppid()
DispatchQueue.global().async {
    while true {
        usleep(100_000)
        if getppid() != parentPID { exit(2) }
    }
}

struct Result: Codable { let text: String; let confidence: Float }
do {
    guard CommandLine.arguments.count == 4,
          let width = Int(CommandLine.arguments[1]), let height = Int(CommandLine.arguments[2]),
          width > 0, height > 0, width <= 2048, height <= 2048,
          let turns = Int(CommandLine.arguments[3]) else { throw NSError(domain: "Invalid crop", code: 1) }
    let expected = width * height * 4
    let data = FileHandle.standardInput.readData(ofLength: expected + 1)
    guard data.count == expected, let provider = CGDataProvider(data: data as CFData),
          let image = CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                              bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
                              provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
          else { throw NSError(domain: "Invalid pixels", code: 2) }
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.recognitionLanguages = ["en-AU", "en-US"]
    request.usesLanguageCorrection = false // Drawing IDs must not be dictionary-corrected.
    request.minimumTextHeight = 0.005
    let orientations: [CGImagePropertyOrientation] = [.up, .right, .down, .left]
    try VNImageRequestHandler(cgImage: image, orientation: orientations[((turns % 4) + 4) % 4]).perform([request])
    let observations = (request.results ?? []).sorted {
        // Vision uses a bottom-left origin; sort lines top-to-bottom, then left-to-right.
        let dy = $0.boundingBox.midY - $1.boundingBox.midY
        if abs(dy) > min($0.boundingBox.height, $1.boundingBox.height) * 0.5 { return dy > 0 }
        return $0.boundingBox.minX < $1.boundingBox.minX
    }
    let candidates = observations.compactMap { $0.topCandidates(1).first }
    let result = Result(text: candidates.map { $0.string }.joined(separator: "\n"),
                        confidence: candidates.isEmpty ? 0 : candidates.map { $0.confidence }.reduce(0, +) / Float(candidates.count))
    FileHandle.standardOutput.write(try JSONEncoder().encode(result))
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8)); exit(1)
}
