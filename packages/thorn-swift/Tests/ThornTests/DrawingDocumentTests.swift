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
