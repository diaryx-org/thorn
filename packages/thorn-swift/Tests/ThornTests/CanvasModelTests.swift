// Drives the canvas with no window: draws into a bitmap context, so the
// view↔user mapping is real, then clicks and drags through the model and
// checks what landed in the document and what the picture painted.

import CoreGraphics
import Foundation
import XCTest

@testable import Thorn

final class CanvasModelTests: XCTestCase {
    // A 200×100 drawing shown in a 400×200 view: 2 points per user unit.
    let scene = """
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100" data-diaryx-drawing="1">
          <rect x="10" y="10" width="40" height="20" fill="#ff0000" data-id="s1"/>
          <circle cx="150" cy="50" r="20" fill="#0000ff" data-id="s2"/>
        </svg>

        """

    func makeContext() -> CGContext {
        let c = CGContext(
            data: nil, width: 400, height: 200, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)!
        // Bitmap contexts are y-up; the canvas draws y-down.
        c.translateBy(x: 0, y: 200)
        c.scaleBy(x: 1, y: -1)
        return c
    }

    func pixel(_ c: CGContext, _ x: Int, _ y: Int) -> (UInt8, UInt8, UInt8) {
        let p = c.data!.assumingMemoryBound(to: UInt8.self)
        let i = y * c.bytesPerRow + x * 4
        return (p[i], p[i + 1], p[i + 2])
    }

    func testPictureDrawsAndClicksMapThroughTheFit() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        XCTAssertNotNil(doc.picture)
        XCTAssertEqual(doc.picture?.size, CGSize(width: 200, height: 100))
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        let (r, g, b) = pixel(context, 40, 40) // user (20, 20): inside the red rect
        XCTAssertEqual([r, g, b], [255, 0, 0])
        XCTAssertEqual(model.userPoint(CGPoint(x: 40, y: 40)), CGPoint(x: 20, y: 20))

        model.beginPointer(at: CGPoint(x: 40, y: 40))
        XCTAssertEqual(model.selection, ["s1"])
        model.pointerDragged(to: CGPoint(x: 60, y: 50)) // +10, +5 user units
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 10, y: 10), "nothing lands mid-drag")
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: "s1"), CGRect(x: 20, y: 15, width: 40, height: 20))
        XCTAssertTrue(doc.source.contains("<rect x=\"20\" y=\"15\" width=\"40\" height=\"20\" fill=\"#ff0000\" data-id=\"s1\"/>"))

        model.beginPointer(at: CGPoint(x: 390, y: 190)) // empty
        XCTAssertEqual(model.selection, [])
        model.pointerUp()
    }

    func testShiftClickSelectsSeveralAndGroupsThem() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)

        model.beginPointer(at: CGPoint(x: 40, y: 40)) // s1
        model.pointerUp()
        model.beginPointer(at: CGPoint(x: 300, y: 100), extending: true) // + s2
        model.pointerUp()
        XCTAssertEqual(model.selection, ["s1", "s2"])
        XCTAssertTrue(model.canGroup)
        XCTAssertFalse(model.canUngroup)

        // Dragging one of them moves both, as one step.
        model.beginPointer(at: CGPoint(x: 40, y: 40))
        model.pointerDragged(to: CGPoint(x: 60, y: 40)) // +10 user units
        model.pointerUp()
        XCTAssertEqual(model.selection, ["s1", "s2"])
        XCTAssertEqual(doc.bounds(id: "s1")?.minX, 20)
        XCTAssertEqual(doc.bounds(id: "s2")?.minX, 140)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.source, scene)

        model.groupSelection()
        XCTAssertEqual(model.selection, ["s3"])
        XCTAssertTrue(model.canUngroup)
        XCTAssertEqual(doc.shape(id: "s3")?.kind, .group)
        // A click on a member selects the group, and moves it by transform.
        model.beginPointer(at: CGPoint(x: 300, y: 100))
        XCTAssertEqual(model.selection, ["s3"])
        model.pointerDragged(to: CGPoint(x: 310, y: 100))
        model.pointerUp()
        XCTAssertTrue(doc.source.contains("<g data-id=\"s3\" transform=\"translate(5 0)\">"))
        XCTAssertEqual(doc.bounds(id: "s1")?.minX, 15)

        model.ungroupSelection()
        XCTAssertEqual(model.selection, ["s1", "s2"])
        XCTAssertNil(doc.shape(id: "s3"))
        XCTAssertEqual(doc.bounds(id: "s1")?.minX, 15, "the transform went down onto the members")
        XCTAssertTrue(try doc.undo()) // ungroup
        XCTAssertTrue(try doc.undo()) // move
        XCTAssertTrue(try doc.undo()) // group
        XCTAssertEqual(doc.source, scene)

        // Shift-click on a selected shape takes it out.
        model.select(["s1", "s2"])
        model.beginPointer(at: CGPoint(x: 40, y: 40), extending: true)
        model.pointerUp()
        XCTAssertEqual(model.selection, ["s2"])
        model.deleteSelection()
        XCTAssertEqual(model.selection, [])
    }

    func testHandleResizeAndCreateLandAsOneStepEach() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)

        model.select("s1")
        // The bottom-right handle of s1 is at user (50, 30) = view (100, 60).
        model.beginPointer(at: CGPoint(x: 101, y: 61))
        model.pointerDragged(to: CGPoint(x: 121, y: 81))
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: "s1"), CGRect(x: 10, y: 10, width: 50, height: 30))

        model.tool = .ellipse
        model.beginPointer(at: CGPoint(x: 200, y: 20))
        model.pointerDragged(to: CGPoint(x: 240, y: 60))
        model.pointerUp()
        XCTAssertEqual(model.tool, .select, "a create tool is one-shot")
        XCTAssertEqual(model.selection, ["s3"])
        XCTAssertEqual(doc.bounds(id: "s3"), CGRect(x: 100, y: 10, width: 20, height: 20))

        model.deleteSelection()
        XCTAssertEqual(model.selection, [], "the selection follows the document")
        XCTAssertTrue(try doc.undo()) // delete
        XCTAssertTrue(try doc.undo()) // create
        XCTAssertTrue(try doc.undo()) // resize
        XCTAssertEqual(doc.source, scene)
        XCTAssertEqual(doc.check().count, 0)
    }
}
