import AppKit
import Foundation
import Vision
import Quartz

struct OcrBlock: Codable {
    let text: String
    let x: Double
    let y: Double
    let width: Double
    let height: Double
    let page: Int
    let source: String
}

func extractNativePDFBoxes(page: PDFPage, pageNumber: Int) -> [OcrBlock] {
    var blocks: [OcrBlock] = []
    let cropBox = page.bounds(for: .cropBox)
    let w = cropBox.width
    let h = cropBox.height
    
    if let fullSelection = page.selection(for: cropBox) {
        let lines = fullSelection.selectionsByLine()
        for line in lines {
            if let text = line.string?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty {
                let b = line.bounds(for: page)
                
                // Normalization: map from absolute points relative to cropBox origin -> percentage
                let nx = (b.minX - cropBox.minX) / w
                
                // PDFKit y is distance from bottom. Standard UI assumes distance from top.
                let ny = (cropBox.maxY - b.maxY) / h 
                
                let nw = b.width / w
                let nh = b.height / h
                
                let block = OcrBlock(
                    text: text,
                    x: Double(nx),
                    y: Double(ny),
                    width: Double(nw),
                    height: Double(nh),
                    page: pageNumber,
                    source: "native"
                )
                blocks.append(block)
            }
        }
    }
    return blocks
}

func extractOCRCGImage(cgImage: CGImage, pageNumber: Int) -> [OcrBlock] {
    var blocks: [OcrBlock] = []
    let requestHandler = VNImageRequestHandler(cgImage: cgImage, options: [:])
    let request = VNRecognizeTextRequest { (request, error) in
        if let error = error {
            fputs("Vision Error: \(error.localizedDescription)\n", stderr)
            return
        }
        guard let observations = request.results as? [VNRecognizedTextObservation] else { return }
        
        for observation in observations {
            guard let topCandidate = observation.topCandidates(1).first else { continue }
            
            // Vision returns normalized bounding boxes [0,1] with origin at bottom-left.
            // Svelte/CSS uses top-left origin.
            let box = observation.boundingBox
            let yFlipped = 1.0 - Double(box.origin.y) - Double(box.size.height)
            
            let block = OcrBlock(
                text: topCandidate.string,
                x: Double(box.origin.x),
                y: yFlipped,
                width: Double(box.size.width),
                height: Double(box.size.height),
                page: pageNumber,
                source: "optical"
            )
            blocks.append(block)
        }
    }
    
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    
    do {
        try requestHandler.perform([request])
    } catch {
        fputs("Failed to perform OCR: \(error.localizedDescription)\n", stderr)
    }
    
    return blocks
}

guard CommandLine.arguments.count > 2 else {
    fputs("Usage: scholium-ocr <path-to-pdf> <page-number-0-indexed>\n", stderr)
    exit(1)
}

let pdfPath = CommandLine.arguments[1]
guard let pageNumber = Int(CommandLine.arguments[2]) else {
    fputs("Invalid page number parameter\n", stderr)
    exit(1)
}

let url = URL(fileURLWithPath: pdfPath)
guard let pdfDoc = PDFDocument(url: url) else {
    fputs("Failed to open PDF document at \(pdfPath)\n", stderr)
    exit(1)
}

guard pageNumber >= 0 && pageNumber < pdfDoc.pageCount else {
    fputs("Page number \(pageNumber) out of bounds.\n", stderr)
    exit(1)
}

guard let page = pdfDoc.page(at: pageNumber) else {
    fputs("Failed to extract page object.\n", stderr)
    exit(1)
}

// 1. Primary Engine Pipeline: Extract PDFKit Native Geometry first
var blocks = extractNativePDFBoxes(page: page, pageNumber: pageNumber)

// 2. Optical Fallback Engine: Only generate CGImage buffer and trigger Vision Neural Engine if purely flattened
if blocks.isEmpty {
    let pageRect = page.bounds(for: .cropBox)
    let rotation = page.rotation

    var logicalWidth = pageRect.width
    var logicalHeight = pageRect.height

    if rotation == 90 || rotation == 270 {
        logicalWidth = pageRect.height
        logicalHeight = pageRect.width
    }

    let targetSize = NSSize(width: logicalWidth * 2.0, height: logicalHeight * 2.0)
    let image = page.thumbnail(of: targetSize, for: .cropBox)

    let cgTargetRect = CGRect(x: 0, y: 0, width: targetSize.width, height: targetSize.height)

    guard let context = CGContext(
        data: nil,
        width: Int(targetSize.width),
        height: Int(targetSize.height),
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else {
        fputs("Failed to allocate CG Context for page \(pageNumber)\n", stderr)
        exit(1)
    }

    context.setFillColor(CGColor(red: 1.0, green: 1.0, blue: 1.0, alpha: 1.0))
    context.fill(cgTargetRect)

    var imageRect = NSZeroRect
    if let rawImage = image.cgImage(forProposedRect: &imageRect, context: nil, hints: nil) {
        context.draw(rawImage, in: cgTargetRect)
    }

    guard let finalCgImage = context.makeImage() else {
        fputs("Failed to finalize bounding Context Image.\n", stderr)
        exit(1)
    }

    blocks = extractOCRCGImage(cgImage: finalCgImage, pageNumber: pageNumber)
}

do {
    let jsonData = try JSONEncoder().encode(blocks)
    if let jsonString = String(data: jsonData, encoding: .utf8) {
        print(jsonString)
    }
} catch {
    fputs("Failed to serialize OCR payload.\n", stderr)
    exit(1)
}
