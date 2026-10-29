//  ThornDocument.swift
//
//  A file on disk, as the document system sees it. `DocumentGroup` owns the
//  rest — Open, Save, Save As, Duplicate, Rename, Revert, autosave, Versions,
//  the recents list and the title bar's proxy icon on the Mac; the document
//  browser and the Files app on iOS — and asks this class only three things:
//  read these bytes, write these bytes, and tell me when something changed.

import SwiftUI
import Thorn
import UniformTypeIdentifiers

/// An SVG drawing, holding the canvas model the window edits.
///
/// A reference document, not a value one: the drawing is a class that owns a
/// live FFI handle, and the canvas edits it in place. That is also why this
/// carries no undo of its own — twig keeps the history, and Edit ▸ Undo
/// reaches it through `DrawingCanvasView.undo(_:)` on the responder chain.
final class ThornDocument: ReferenceFileDocument {
    typealias Snapshot = String

    /// Every SVG opens: a drawing of the profile edits cleanly, any other is
    /// shown and `check()` says what it lacks.
    static var readableContentTypes: [UTType] { [.svg] }
    static var writableContentTypes: [UTType] { [.svg] }

    /// The canvas over the drawing: the tool, the selection, the zoom. Owned
    /// here rather than by the view so that a window rebuilt by SwiftUI keeps
    /// its place in the picture.
    let model: CanvasModel

    /// A new drawing: the profile's template.
    init() {
        model = CanvasModel(document: DrawingDocument.fresh())
    }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents,
              let source = String(data: data, encoding: .utf8)
        else { throw CocoaError(.fileReadCorruptFile) }
        model = CanvasModel(document: try DrawingDocument(source: source))
    }

    /// Serialised on the main thread while the model is quiet; the document
    /// system then writes it from wherever it likes.
    func snapshot(contentType: UTType) throws -> String {
        model.document.source
    }

    func fileWrapper(snapshot: String, configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(snapshot.utf8))
    }
}
