import Quartz

let pdfDoc = PDFDocument(url: URL(fileURLWithPath: "dummy.pdf"))!
let page = pdfDoc.page(at: 0)!
let cropBox = page.bounds(for: .cropBox)

print("CropBox:", cropBox)

if let selection = page.selection(for: cropBox) {
    let lines = selection.selectionsByLine()
    for line in lines {
        if let text = line.string {
            let bounds = line.bounds(for: page)
            print("Line:", text)
            print("Bounds:", bounds)
        }
    }
}
