// Drives a real drawing through the Rust binding: the Swift side of the
// promise that every gesture is one splice and one undo step.

import Foundation
import XCTest

@testable import Thorn

final class DrawingDocumentTests: XCTestCase {
    let empty = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\" data-diaryx-drawing=\"1\">\n</svg>\n"

    func testAddDeleteUndoRoundTrip() throws {
        let doc = try DrawingDocument(source: empty)
        XCTAssertEqual(doc.check().count, 0)
        let id = try doc.addRect(CGRect(x: 10, y: 10.5, width: 80, height: 40))
        XCTAssertEqual(id, "s1")
        XCTAssertEqual(doc.shapes.count, 1)
        XCTAssertEqual(doc.shapes[0].kind, .rect)
        XCTAssertTrue(doc.source.contains("<rect x=\"10\" y=\"10.5\" width=\"80\" height=\"40\" data-id=\"s1\"/>"))
        try doc.delete(id: id)
        XCTAssertEqual(doc.shapes.count, 0)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.shapes.count, 1)
        XCTAssertTrue(try doc.undo())
        XCTAssertEqual(doc.source, empty)
        XCTAssertFalse(try doc.undo())
    }

    func testMoveResizeReorder() throws {
        let doc = try DrawingDocument(source: empty)
        let a = try doc.addRect(CGRect(x: 0, y: 0, width: 10, height: 10))
        let b = try doc.addRect(CGRect(x: 20, y: 20, width: 10, height: 10))
        try doc.move(id: a, by: CGVector(dx: 5, dy: 2.5))
        XCTAssertEqual(doc.bounds(id: a), CGRect(x: 5, y: 2.5, width: 10, height: 10))
        try doc.resize(id: b, to: CGRect(x: 1, y: 1, width: 2, height: 3))
        XCTAssertEqual(doc.bounds(id: b), CGRect(x: 1, y: 1, width: 2, height: 3))
        XCTAssertTrue(try doc.reorder(id: a, .toFront))
        XCTAssertEqual(doc.shapes.map(\.id), [b, a])
        XCTAssertFalse(try doc.reorder(id: a, .forward))
        for _ in 0..<5 { XCTAssertTrue(try doc.undo()) }
        XCTAssertEqual(doc.source, empty)
    }

    func testGroupUngroupAndAPathThroughATransform() throws {
        let doc = try DrawingDocument(source: empty)
        let a = try doc.addRect(CGRect(x: 0, y: 0, width: 10, height: 10))
        let b = try doc.addLine(from: CGPoint(x: 20, y: 20), to: CGPoint(x: 30, y: 30))
        let g = try doc.group(ids: [a, b])
        XCTAssertEqual(g, "s3")
        XCTAssertEqual(doc.shapes.map(\.id), [g, a, b])
        XCTAssertEqual(doc.shape(id: a)?.group, g)
        XCTAssertEqual(doc.outermost(id: b)?.id, g)
        XCTAssertEqual(doc.members(id: g).map(\.id), [a, b])
        XCTAssertEqual(doc.bounds(id: g), CGRect(x: 0, y: 0, width: 30, height: 30))
        try doc.move(id: g, by: CGVector(dx: 5, dy: 5))
        XCTAssertTrue(doc.source.contains("<g data-id=\"s3\" transform=\"translate(5 5)\">"))
        XCTAssertEqual(doc.bounds(id: a), CGRect(x: 5, y: 5, width: 10, height: 10), "a member's box is through the group")
        XCTAssertEqual(doc.hit(CGPoint(x: 7, y: 7), tolerance: 0)?.id, a)
        XCTAssertEqual(try doc.ungroup(id: g), [a, b])
        XCTAssertEqual(doc.bounds(id: a), CGRect(x: 5, y: 5, width: 10, height: 10), "nothing moved")
        XCTAssertNil(doc.shape(id: g))
        XCTAssertThrowsError(try doc.ungroup(id: a))
        // group, move, ungroup: three steps, then two adds.
        for _ in 0..<5 { XCTAssertTrue(try doc.undo()) }
        XCTAssertEqual(doc.source, empty)

        let path = try DrawingDocument(source: "<svg viewBox=\"0 0 9 9\"><path d=\"M1 1 h4 v4 z\" data-id=\"p\"/></svg>")
        XCTAssertEqual(path.bounds(id: "p"), CGRect(x: 1, y: 1, width: 4, height: 4))
        XCTAssertEqual(path.hit(CGPoint(x: 4, y: 2), tolerance: 0)?.id, "p")
        try path.resize(id: "p", to: CGRect(x: 0, y: 0, width: 8, height: 8))
        XCTAssertEqual(path.bounds(id: "p"), CGRect(x: 0, y: 0, width: 8, height: 8))
    }

    func testALabelIsMeasuredByTheFontItIsDrawnWith() throws {
        let doc = try DrawingDocument(source: empty)
        let t = try doc.addText("Hello", at: CGPoint(x: 10, y: 50))
        let b = try XCTUnwrap(doc.bounds(id: t))
        // The box is the glyphs': the H starts at the anchor, the cap
        // height stands above the baseline, nothing hangs far below it.
        XCTAssertEqual(b.minX, 10, accuracy: 2)
        XCTAssertGreaterThan(b.width, 20)
        XCTAssertLessThan(b.width, 40)
        XCTAssertLessThan(b.minY, 50)
        XCTAssertEqual(b.maxY, 52, accuracy: 3)
        XCTAssertEqual(doc.hit(CGPoint(x: b.midX, y: b.midY), tolerance: 0)?.id, t)
        XCTAssertEqual(doc.shape(id: t)?.text, "Hello")

        // A stylesheet's size is the layout's to report.
        let styled = try DrawingDocument(source: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 9 9\"><style>text { font: 16px sans-serif }</style><text x=\"1\" y=\"2\" data-id=\"t\">Hi</text></svg>")
        XCTAssertEqual(styled.fontSize(id: "t"), 16)
        XCTAssertEqual(styled.fontSize(id: nil), 16, "a new label's too")
    }

    func testAnArrowBoundToAShapeFollowsIt() throws {
        let doc = try DrawingDocument(source: empty)
        let r = try doc.addRect(CGRect(x: 10, y: 10, width: 20, height: 20))
        let l = try doc.addLine(from: CGPoint(x: 100, y: 20), to: CGPoint(x: 90, y: 20))
        try doc.bind(id: l, .to, to: r)
        XCTAssertEqual(doc.binding(id: l, .to), r)
        XCTAssertEqual(doc.endPoint(id: l, .to), CGPoint(x: 30, y: 20), "on the rect's right edge")
        // The rect moves down: the end leaves its right edge on the way
        // to the other end, lower now.
        try doc.move(id: r, by: CGVector(dx: 0, dy: 30))
        XCTAssertEqual(doc.endPoint(id: l, .to), CGPoint(x: 30, y: 46.25))
        XCTAssertTrue(doc.source.contains("<line x1=\"100\" y1=\"20\" x2=\"30\" y2=\"46.25\" data-id=\"s2\" data-to=\"s1\"/>"))
        try doc.delete(id: r)
        XCTAssertNil(doc.binding(id: l, .to))
        XCTAssertThrowsError(try doc.bind(id: r, .to, to: l), "gone")
        XCTAssertEqual(doc.check().count, 0)
    }

    func testRefusesWhatIsNotAnSvg() {
        XCTAssertThrowsError(try DrawingDocument(source: "<html/>"))
    }

    func testAFileOffTheProfileStillOpens() throws {
        let doc = try DrawingDocument(source: "<svg><rect x=\"1.0\"/></svg>")
        let rules = doc.check().map(\.rule)
        XCTAssertEqual(rules.count, 4)
        XCTAssertTrue(rules[0].contains("data-diaryx-drawing"))
    }
}
