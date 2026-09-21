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
        model.pointerDragged(to: CGPoint(x: 50, y: 45)) // +5, +2.5 user units
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 15, y: 12.5), "the picture follows the drag")
        model.pointerDragged(to: CGPoint(x: 60, y: 50)) // +10, +5
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: "s1"), CGRect(x: 20, y: 15, width: 40, height: 20))
        XCTAssertTrue(doc.source.contains("<rect x=\"20\" y=\"15\" width=\"40\" height=\"20\" fill=\"#ff0000\" data-id=\"s1\"/>"))
        XCTAssertTrue(try doc.undo(), "one step for the whole drag")
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 10, y: 10))
        XCTAssertFalse(try doc.undo())
        XCTAssertTrue(try doc.redo())

        model.beginPointer(at: CGPoint(x: 390, y: 190)) // empty
        XCTAssertEqual(model.selection, [])
        model.pointerUp()
    }

    /// The appearance is the canvas's, not the file's: dark mode draws
    /// the template's `currentColor` ink light on a dark sheet, a colour a
    /// shape spells out stays its own, and the bytes never change.
    func testAppearanceRecoloursTheInkAndNotTheFile() throws {
        let doc = DrawingDocument.fresh()
        _ = try doc.addRect(CGRect(x: 0, y: 0, width: 200, height: 100)) // fits the page around it
        let before = doc.source
        XCTAssertTrue(before.contains("color=\"#222\""))
        let model = CanvasModel(document: doc)
        model.setZoom(1, about: .zero)
        let context = makeContext()
        let view = CGRect(x: 0, y: 0, width: 400, height: 200)

        model.draw(in: context, rect: view, scale: 1)
        let edge = model.viewPoint(CGPoint(x: 0, y: 50)) // on the rect's left stroke
        let inside = model.viewPoint(CGPoint(x: 100, y: 50))
        let (r, _, _) = pixel(context, Int(edge.x), Int(edge.y))
        XCTAssertEqual(r, 0x22, "light: the stroke is the root's #222")
        XCTAssertEqual(pixel(context, Int(inside.x), Int(inside.y)).0, 255, "on a white sheet")

        model.appearance = .dark
        model.draw(in: context, rect: view, scale: 1)
        let (dr, _, _) = pixel(context, Int(edge.x), Int(edge.y))
        XCTAssertEqual(dr, 0xe6, "dark: the same stroke is light ink")
        XCTAssertLessThan(pixel(context, Int(inside.x), Int(inside.y)).0, 0x40, "on a dark sheet")
        XCTAssertEqual(doc.source, before, "and the file is as it was")
        XCTAssertTrue(try doc.undo(), "the rect's step is the only one")
        XCTAssertFalse(try doc.undo())
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
        model.pointerDragged(to: CGPoint(x: 220, y: 40))
        XCTAssertEqual(doc.bounds(id: "s3"), CGRect(x: 100, y: 10, width: 10, height: 10), "drawn as it is dragged out")
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

    func testKeysPickToolsAndTheLockKeepsOne() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)

        // Every tool's keys pick it.
        for tool in Tool.all {
            for key in tool.keys {
                model.tool = .select
                XCTAssertTrue(model.key(key), "\(key)")
                XCTAssertEqual(model.tool, tool, "\(key)")
            }
        }
        XCTAssertEqual(Tool.forKey("R"), .rect, "case-insensitive")
        model.tool = .select
        XCTAssertFalse(model.key("z"))

        // A create tool is one-shot: one rectangle, then select.
        model.key("r")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 140, y: 120))
        model.pointerUp()
        XCTAssertEqual(doc.shapes.count, 3)
        XCTAssertEqual(model.tool, .select)

        // Locked, it stays.
        model.key(Tool.lockKey)
        XCTAssertTrue(model.locked)
        model.key("2")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 140, y: 120))
        model.pointerUp()
        XCTAssertEqual(doc.shapes.count, 4)
        XCTAssertEqual(model.tool, .rect, "kept by the lock")
        XCTAssertEqual(model.selection.count, 1)

        // Escape: select, nothing selected.
        model.key("\u{1B}")
        XCTAssertEqual(model.tool, .select)
        XCTAssertEqual(model.selection, [])
    }

    func testThePageFollowsAShapeDraggedOffItAndNothingOnScreenMoves() throws {
        // A square page in a wide view: fitted at 2 points per unit, it
        // spans view x 100…300, with desk either side.
        let doc = try DrawingDocument(source: """
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100" data-diaryx-drawing="1">
              <rect x="10" y="10" width="40" height="20" fill="#ff0000" data-id="s1"/>
              <circle cx="70" cy="50" r="15" fill="#0000ff" data-id="s2"/>
            </svg>

            """)
        let model = CanvasModel(document: doc)
        let view = CGRect(x: 0, y: 0, width: 400, height: 200)
        var context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        XCTAssertEqual(model.fitted, CGRect(x: 0, y: 0, width: 100, height: 100))
        XCTAssertEqual(model.userPoint(CGPoint(x: 240, y: 100)), CGPoint(x: 70, y: 50))
        let desk = pixel(context, 10, 10)
        XCTAssertNotEqual([desk.0, desk.1, desk.2], [255, 255, 255], "the desk is not the sheet")
        var (r, g, b) = pixel(context, 110, 10)
        XCTAssertEqual([r, g, b], [255, 255, 255], "the sheet")
        (r, g, b) = pixel(context, 240, 100)
        XCTAssertEqual([r, g, b], [0, 0, 255])

        // The circle dragged 30 units right, half of it past the page's
        // edge: the page grows to hold it, the mapping stays where it
        // was, the rect stays where it was, and the circle is painted
        // where it went rather than cut at the old edge.
        model.beginPointer(at: CGPoint(x: 240, y: 100))
        XCTAssertEqual(model.selection, ["s2"])
        model.pointerDragged(to: CGPoint(x: 300, y: 100))
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: "s2"), CGRect(x: 85, y: 35, width: 30, height: 30))
        XCTAssertEqual(doc.page, CGRect(x: -6, y: -6, width: 137, height: 87), "the shapes' box, 16 out")
        XCTAssertTrue(doc.source.contains("viewBox=\"-6 -6 137 87\" width=\"137\" height=\"87\""))
        XCTAssertEqual(model.fitted, CGRect(x: 0, y: 0, width: 100, height: 100), "pinned until a zoom to fit")
        context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(CGPoint(x: 240, y: 100)), CGPoint(x: 70, y: 50), "nothing on screen moved")
        (r, g, b) = pixel(context, 140, 40)
        XCTAssertEqual([r, g, b], [255, 0, 0], "the rect, where it was")
        (r, g, b) = pixel(context, 320, 100)
        XCTAssertEqual([r, g, b], [0, 0, 255], "the circle, past the old edge")
        (r, g, b) = pixel(context, 95, 100)
        XCTAssertEqual([r, g, b], [255, 255, 255], "the sheet reaches the new page's edge")
        (r, g, b) = pixel(context, 10, 10)
        XCTAssertEqual([r, g, b], [desk.0, desk.1, desk.2])
        XCTAssertTrue(try doc.undo(), "the move and the page are one step")
        XCTAssertEqual(doc.page, CGRect(x: 0, y: 0, width: 100, height: 100))
        XCTAssertTrue(try doc.redo())

        // Cmd-0 fits the page as it stands now.
        model.zoomToFit()
        XCTAssertEqual(model.fitted, CGRect(x: -6, y: -6, width: 137, height: 87))
        context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        let scale = min(400 / 137.0, 200 / 87.0)
        XCTAssertEqual(model.viewPoint(CGPoint(x: -6, y: -6)).y, (200 - 87 * scale) / 2, accuracy: 1e-9)
        let centre = model.viewPoint(CGPoint(x: 100, y: 50))
        (r, g, b) = pixel(context, Int(centre.x), Int(centre.y))
        XCTAssertEqual([r, g, b], [0, 0, 255])
    }

    func testAPinchZoomsAboutItsPointAndTouchesNothing() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let view = CGRect(x: 0, y: 0, width: 400, height: 200)
        model.draw(in: makeContext(), rect: view, scale: 1)
        XCTAssertEqual(model.zoom, 1)
        var told: [CGFloat] = []
        model.onZoomChange = { told.append($0) }

        // Zooming in twice about view (40, 40) — over user (20, 20) — keeps
        // that user point under the finger, and the picture follows.
        model.zoom(by: 2, about: CGPoint(x: 40, y: 40))
        var context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        XCTAssertEqual(model.zoom, 2)
        XCTAssertEqual(told, [2])
        XCTAssertEqual(model.pan, CGVector(dx: -40, dy: -40))
        XCTAssertEqual(model.userPoint(CGPoint(x: 40, y: 40)), CGPoint(x: 20, y: 20))
        XCTAssertEqual(model.userPoint(CGPoint(x: 44, y: 40)), CGPoint(x: 21, y: 20), "4 points per user unit now")
        XCTAssertEqual(model.viewPoint(CGPoint(x: 50, y: 30)), CGPoint(x: 160, y: 80), "the rect's corner, four times out from the fixed point")
        var (r, g, b) = pixel(context, 159, 79)
        XCTAssertEqual([r, g, b], [255, 0, 0], "the picture is drawn under the zoom")
        (r, g, b) = pixel(context, 161, 81)
        XCTAssertNotEqual([r, g, b], [255, 0, 0])
        XCTAssertFalse(try doc.undo(), "zoom is the view's, not the document's")

        // A click still maps through the zoomed fit: user (20, 20) hits s1,
        // and a drag of 8 points moves it 2 units.
        model.beginPointer(at: CGPoint(x: 40, y: 40))
        XCTAssertEqual(model.selection, ["s1"])
        model.pointerDragged(to: CGPoint(x: 48, y: 40))
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 12, y: 10))
        XCTAssertTrue(try doc.undo())

        // A step out about the middle keeps what was under it, then back
        // to the fit.
        let middle = CGPoint(x: 200, y: 100)
        XCTAssertEqual(model.userPoint(middle), CGPoint(x: 60, y: 35))
        model.zoomOut()
        XCTAssertEqual(model.zoom, 2 / 2.squareRoot(), accuracy: 1e-9)
        model.draw(in: makeContext(), rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(middle).x, 60, accuracy: 1e-9, "the middle stayed put")
        XCTAssertEqual(model.userPoint(middle).y, 35, accuracy: 1e-9)
        model.zoomToFit()
        XCTAssertEqual(model.zoom, 1)
        XCTAssertEqual(model.pan, .zero)
        context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(CGPoint(x: 40, y: 40)), CGPoint(x: 20, y: 20))

        // Clamped at both ends.
        model.zoom(by: 1000, about: .zero)
        XCTAssertEqual(model.zoom, CanvasModel.zoomRange.upperBound)
        model.zoom(by: 1e-6, about: .zero)
        XCTAssertEqual(model.zoom, CanvasModel.zoomRange.lowerBound)
        model.zoomToFit()

        // A scroll slides the picture as the hand does.
        model.pan(by: CGVector(dx: 20, dy: 10))
        XCTAssertEqual(model.pan, CGVector(dx: 20, dy: 10))
        model.draw(in: makeContext(), rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(CGPoint(x: 60, y: 50)), CGPoint(x: 20, y: 20))
    }

    func testACancelledDragLandsNothing() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)

        // A move under way, previewed in the document, is undone.
        model.beginPointer(at: CGPoint(x: 40, y: 40))
        model.pointerDragged(to: CGPoint(x: 60, y: 50))
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 20, y: 15))
        model.cancelPointer()
        XCTAssertEqual(doc.bounds(id: "s1")?.origin, CGPoint(x: 10, y: 10))
        XCTAssertFalse(try doc.undo(), "and left no step behind")

        // So is a shape being created.
        model.key("r")
        model.beginPointer(at: CGPoint(x: 200, y: 20))
        model.pointerDragged(to: CGPoint(x: 240, y: 60))
        XCTAssertEqual(doc.shapes.count, 3)
        model.cancelPointer()
        XCTAssertEqual(doc.shapes.count, 2)
        XCTAssertEqual(model.tool, .rect, "the tool is kept: nothing was made")
        model.pointerUp()
        XCTAssertEqual(doc.shapes.count, 2, "and a stray up after the cancel does nothing")
    }

    func testTheHandPansAndTheEraserSweepsAsOneStep() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        let view = CGRect(x: 0, y: 0, width: 400, height: 200)
        model.draw(in: makeContext(), rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(CGPoint(x: 40, y: 40)), CGPoint(x: 20, y: 20))

        // Dragging with the hand moves the picture under the pointer,
        // and touches nothing in the document.
        model.key("h")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 120, y: 110))
        model.pointerUp()
        XCTAssertEqual(model.pan, CGVector(dx: 20, dy: 10))
        XCTAssertEqual(model.tool, .hand, "the hand is not one-shot")
        let context = makeContext()
        model.draw(in: context, rect: view, scale: 1)
        XCTAssertEqual(model.userPoint(CGPoint(x: 60, y: 50)), CGPoint(x: 20, y: 20), "user (20,20) is now 20 points right and 10 down")
        let (r, g, b) = pixel(context, 60, 50)
        XCTAssertEqual([r, g, b], [255, 0, 0], "and the picture is drawn there")
        XCTAssertFalse(try doc.undo(), "nothing to undo")

        // The eraser deletes what it passes over, as one step, when it lifts.
        model.key("e")
        model.beginPointer(at: CGPoint(x: 60, y: 50)) // the rect
        XCTAssertEqual(doc.shapes.count, 2, "not yet")
        model.pointerDragged(to: CGPoint(x: 200, y: 100)) // between
        model.pointerDragged(to: CGPoint(x: 320, y: 110)) // the circle: user (150, 50)
        model.pointerUp()
        XCTAssertEqual(doc.shapes.count, 0)
        XCTAssertEqual(model.tool, .eraser)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.shapes.count, 2, "one step for the sweep")
    }

    func testTheArrowToolDrawsALineWithAHead() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        model.key("a")
        XCTAssertEqual(model.tool, .arrow)
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 200, y: 100))
        model.pointerUp()
        let id = try XCTUnwrap(model.selection.first)
        XCTAssertEqual(doc.heads(id: id), .end)
        XCTAssertEqual(doc.shape(id: id)?.kind, .line, "an arrow is a line the canvas drags by its ends")
        XCTAssertTrue(doc.source.contains("<line x1=\"50\" y1=\"50\" x2=\"100\" y2=\"50\" data-arrow=\"end\" data-id=\"\(id)\"/>"))
        XCTAssertNil(doc.heads(id: "s1"))
        XCTAssertTrue(try doc.undo(), "one step")
    }

    func testALineIsBentByItsMidpointHandleAndStraightenedByDraggingItBack() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        model.key("l")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 200, y: 100))
        model.pointerUp()
        let id = try XCTUnwrap(model.selection.first)
        let straight = try XCTUnwrap(doc.connector(id: id))
        XCTAssertNil(straight.control)
        XCTAssertEqual(straight.midpoint.cgPoint, CGPoint(x: 75, y: 50), "the handle is half-way along, in user units")

        // The midpoint handle, at view (150, 100), dragged down 40 view
        // points — 20 user units — bends the line into a path through there.
        model.beginPointer(at: CGPoint(x: 150, y: 100))
        model.pointerDragged(to: CGPoint(x: 150, y: 140))
        model.pointerUp()
        XCTAssertEqual(model.selection, [id], "still selected")
        let bent = try XCTUnwrap(doc.connector(id: id))
        XCTAssertEqual(doc.shape(id: id)?.kind, .path)
        XCTAssertEqual(bent.midpoint.cgPoint, CGPoint(x: 75, y: 70))
        XCTAssertEqual(bent.control?.cgPoint, CGPoint(x: 75, y: 90))
        XCTAssertTrue(doc.source.contains("<path d=\"M50 50 Q75 90 100 50\" data-id=\"\(id)\"/>"), doc.source)
        XCTAssertTrue(try doc.undo(), "one step")
        XCTAssertEqual(doc.shape(id: id)?.kind, .line)
        XCTAssertTrue(try doc.redo())

        // Its ends are still its handles, and the box handles are not.
        model.beginPointer(at: CGPoint(x: 200, y: 100))
        model.pointerDragged(to: CGPoint(x: 240, y: 100))
        model.pointerUp()
        XCTAssertEqual(doc.connector(id: id)?.to.cgPoint, CGPoint(x: 120, y: 50))
        XCTAssertEqual(doc.connector(id: id)?.control?.cgPoint, CGPoint(x: 75, y: 90), "the bend stays")
        XCTAssertTrue(try doc.undo())

        // Dragged back onto the chord, it is a line again.
        model.beginPointer(at: CGPoint(x: 150, y: 140))
        model.pointerDragged(to: CGPoint(x: 150, y: 102))
        model.pointerUp()
        XCTAssertEqual(doc.shape(id: id)?.kind, .line)
        XCTAssertTrue(doc.source.contains("<line x1=\"50\" y1=\"50\" x2=\"100\" y2=\"50\" data-id=\"\(id)\"/>"), doc.source)
        XCTAssertNil(doc.connector(id: id)?.control)
    }

    func testTheDrawToolInksAStrokeAsOneStep() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        model.inkWidth = 4
        model.key("p")
        XCTAssertEqual(model.tool, .draw)
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 140, y: 120))
        XCTAssertEqual(doc.shapes.count, 3, "the stroke follows the hand")
        model.pointerDragged(to: CGPoint(x: 140.2, y: 120.1)) // noise, dropped
        model.pointerDragged(to: CGPoint(x: 200, y: 100))
        model.pointerUp()
        let id = try XCTUnwrap(model.selection.first)
        let stroke = try XCTUnwrap(doc.shape(id: id))
        XCTAssertEqual(stroke.kind, .path)
        XCTAssertTrue(doc.source.contains("data-ink=\"monoline\" data-centreline=\"M50 50 L70 60 L100 50\" data-widths=\"4\" data-id=\"\(id)\""), doc.source)
        XCTAssertEqual(model.tool, .select)
        XCTAssertTrue(try doc.undo(), "one step")
        XCTAssertEqual(doc.shapes.count, 2)
        XCTAssertFalse(try doc.undo())

        // A click alone is a dot.
        model.key("7")
        model.beginPointer(at: CGPoint(x: 300, y: 150))
        model.pointerUp()
        XCTAssertEqual(doc.shapes.count, 3)
        XCTAssertTrue(doc.source.contains("data-centreline=\"M150 75\""))
    }

    func testTheDiamondAndNoteToolsAndANoteIsTypedIntoResizedAndEmptiedAsOne() throws {
        let doc = try DrawingDocument(source: scene)
        let model = CanvasModel(document: doc)
        model.draw(in: makeContext(), rect: CGRect(x: 0, y: 0, width: 400, height: 200), scale: 1)
        var edits: [TextEdit] = []
        model.onTextEdit = { edits.append($0) }

        model.key("d")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 200, y: 180))
        model.pointerUp()
        let d = try XCTUnwrap(model.selection.first)
        XCTAssertEqual(doc.shape(id: d)?.kind, .polygon)
        XCTAssertTrue(doc.source.contains("<polygon points=\"75,50 100,70 75,90 50,70\""), doc.source)
        XCTAssertTrue(try doc.undo())

        // A note dragged out opens its label at once; what is typed lands
        // wrapped to the box.
        model.key("n")
        model.beginPointer(at: CGPoint(x: 100, y: 100))
        model.pointerDragged(to: CGPoint(x: 300, y: 180))
        model.pointerUp()
        let n = try XCTUnwrap(model.selection.first)
        let note = try XCTUnwrap(doc.note(id: n))
        XCTAssertEqual(edits.count, 1)
        XCTAssertEqual(edits[0].id, note.label)
        XCTAssertTrue(edits[0].wraps)
        XCTAssertEqual(edits[0].frame, CGRect(x: 116, y: 116, width: 168, height: 48), "the box inside its padding, in view points")
        model.commitTextEdit(edits[0], text: "Words in a box")
        XCTAssertEqual(doc.shape(id: note.label)?.text, "Words in a box")
        XCTAssertEqual(doc.width(id: note.label), 84)

        // Resized by its handle, the label follows and re-wraps: one step.
        model.select(n)
        model.beginPointer(at: CGPoint(x: 300, y: 180)) // bottom-right handle
        model.pointerDragged(to: CGPoint(x: 400, y: 180))
        model.pointerUp()
        XCTAssertEqual(doc.bounds(id: note.frame)?.width, 150)
        XCTAssertEqual(doc.width(id: note.label), 134)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.bounds(id: note.frame)?.width, 100)

        // A double-click anywhere on the note edits its label; emptied,
        // the whole note goes.
        model.doubleClick(at: CGPoint(x: 110, y: 170))
        XCTAssertEqual(edits.count, 2)
        XCTAssertEqual(edits[1].id, note.label)
        XCTAssertEqual(edits[1].text, "Words in a box")
        model.commitTextEdit(edits[1], text: "  ")
        XCTAssertNil(doc.shape(id: n))
        XCTAssertNil(doc.shape(id: note.frame))
    }
}
