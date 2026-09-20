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
