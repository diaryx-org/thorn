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

    func testALineIsDraggedByItsEndsAndBindsToWhatItIsDroppedOn() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)

        model.tool = .line
        model.beginPointer(at: CGPoint(x: 140, y: 100)) // user (70, 50)
        model.pointerDragged(to: CGPoint(x: 200, y: 100)) // user (100, 50)
        model.pointerUp()
        XCTAssertEqual(model.selection, ["s3"])
        XCTAssertEqual(doc.endPoint(id: "s3", .to), CGPoint(x: 100, y: 50))

        // Drag the `to` end onto the circle: it binds, and sits on the rim
        // facing the other end.
        model.beginPointer(at: CGPoint(x: 201, y: 101))
        model.pointerDragged(to: CGPoint(x: 300, y: 100)) // the circle's centre
        model.pointerUp()
        XCTAssertEqual(doc.binding(id: "s3", .to), "s2")
        XCTAssertEqual(doc.endPoint(id: "s3", .to), CGPoint(x: 130, y: 50))
        XCTAssertNil(doc.binding(id: "s3", .from))

        // The circle moves; the end follows.
        model.select("s2")
        model.beginPointer(at: CGPoint(x: 300, y: 100))
        model.pointerDragged(to: CGPoint(x: 300, y: 140)) // +20 user units down
        model.pointerUp()
        let end = try XCTUnwrap(doc.endPoint(id: "s3", .to))
        XCTAssertEqual(hypot(end.x - 150, end.y - 70), 20, accuracy: 0.05)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.endPoint(id: "s3", .to), CGPoint(x: 130, y: 50), "one step")

        // Dropped on nothing, the end unbinds and stays where it fell.
        model.select("s3")
        model.beginPointer(at: CGPoint(x: 260, y: 100)) // user (130, 50)
        model.pointerDragged(to: CGPoint(x: 260, y: 180)) // user (130, 90)
        model.pointerUp()
        XCTAssertNil(doc.binding(id: "s3", .to))
        XCTAssertEqual(doc.endPoint(id: "s3", .to), CGPoint(x: 130, y: 90))
    }

    func testALabelIsTypedIntoAFieldAndReWordedByDoubleClick() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        var edits: [TextEdit] = []
        model.onTextEdit = { edits.append($0) }

        // The label tool opens an empty field at the click; what is typed
        // becomes the label, in one step. Nothing typed, nothing added.
        model.tool = .text()
        model.beginPointer(at: CGPoint(x: 100, y: 100)) // user (50, 50)
        model.pointerUp()
        XCTAssertEqual(model.tool, .select)
        let fresh = try XCTUnwrap(edits.last)
        XCTAssertNil(fresh.id)
        XCTAssertEqual(fresh.anchor, CGPoint(x: 50, y: 50))
        XCTAssertEqual(fresh.fontSize, 24, "12 user units — usvg's default, with no stylesheet — at 2 points each")
        XCTAssertEqual(doc.fontSize(id: nil), 12)
        model.commitTextEdit(fresh, text: "  ")
        XCTAssertEqual(doc.shapes.count, 2)
        model.commitTextEdit(fresh, text: "Hello")
        XCTAssertEqual(model.selection, ["s3"])
        XCTAssertTrue(doc.source.contains("<text x=\"50\" y=\"50\" data-id=\"s3\">Hello</text>"))

        // A double-click on it opens it with its words, over its box.
        let box = try XCTUnwrap(doc.bounds(id: "s3"))
        model.doubleClick(at: model.viewPoint(CGPoint(x: box.midX, y: box.midY)))
        let again = try XCTUnwrap(edits.last)
        XCTAssertEqual(again.id, "s3")
        XCTAssertEqual(again.text, "Hello")
        XCTAssertEqual(again.frame.origin, model.viewPoint(box.origin))
        model.commitTextEdit(again, text: "Hello")
        model.commitTextEdit(again, text: "Bye")
        XCTAssertTrue(doc.source.contains(">Bye</text>"))
        XCTAssertTrue(try doc.undo(), "an unchanged commit wrote nothing")
        XCTAssertTrue(doc.source.contains(">Hello</text>"))
        // Emptied, the label goes.
        model.commitTextEdit(again, text: "")
        XCTAssertNil(doc.shape(id: "s3"))
        XCTAssertEqual(model.selection, [])
        // A double-click on nothing, or on a box, opens nothing.
        model.doubleClick(at: CGPoint(x: 40, y: 40))
        XCTAssertEqual(edits.count, 2)
    }

    func testALabelWrapsToTheWidthItsHandleIsDraggedTo() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let context = makeContext()
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        let t = try doc.addText("the quick brown fox jumps over the lazy dog", at: CGPoint(x: 20, y: 60))
        let one = try XCTUnwrap(doc.bounds(id: t))
        XCTAssertNil(doc.width(id: t))

        // Drag the right-hand handle in to half the width: the label wraps.
        model.select(t)
        model.draw(in: context, rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        let right = model.viewPoint(CGPoint(x: one.maxX, y: one.midY))
        model.beginPointer(at: right)
        model.pointerDragged(to: CGPoint(x: right.x - one.width, y: right.y)) // half, in view points
        model.pointerUp()
        let width = try XCTUnwrap(doc.width(id: t))
        XCTAssertEqual(width, one.width / 2, accuracy: 0.01)
        XCTAssertGreaterThanOrEqual(doc.source.components(separatedBy: "<tspan").count - 1, 2)
        let wrapped = try XCTUnwrap(doc.bounds(id: t))
        XCTAssertEqual(wrapped.minX, one.minX, accuracy: 0.5, "the box stays at the anchor")
        XCTAssertLessThanOrEqual(wrapped.width, width + 0.5)
        XCTAssertGreaterThan(wrapped.height, one.height * 1.8)
        XCTAssertEqual(doc.shape(id: t)?.text, "the quick brown fox jumps over the lazy dog")

        // Editing opens a wrapping field the label's width.
        var edits: [TextEdit] = []
        model.onTextEdit = { edits.append($0) }
        model.editText(id: t)
        let edit = try XCTUnwrap(edits.last)
        XCTAssertTrue(edit.wraps)
        XCTAssertEqual(edit.frame.width, width * 2, "user units at 2 points each")
        model.commitTextEdit(edit, text: "short")
        XCTAssertTrue(doc.source.contains("<tspan x=\"20\">short</tspan></text>"))

        XCTAssertTrue(try doc.undo()) // re-word
        XCTAssertTrue(try doc.undo()) // wrap
        XCTAssertNil(doc.width(id: t))
        XCTAssertEqual(doc.bounds(id: t), one)
    }
}
