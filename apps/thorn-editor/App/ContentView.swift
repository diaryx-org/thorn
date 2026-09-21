//  ContentView.swift
//
//  One document's window: the package's `DrawingEditor` — the canvas, the
//  toolbar, the options strip — over the model its `ThornDocument` owns. This
//  file is only the wiring between the two.

import SwiftUI
import Thorn

struct ContentView: View {
    let document: ThornDocument

    /// The scene's undo manager is the document's: telling it of a change is
    /// how a `ReferenceFileDocument` says it is edited, which is what enables
    /// Save, shows the dot in the close button, and starts the autosave clock.
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        DrawingEditor(model: document.model)
            .onAppear(perform: wire)
            .onChange(of: undoManager) { _ in wire() }
    }

    /// The host's half of the model: what to do when a gesture lands.
    private func wire() {
        let model = document.model
        model.onDocumentChange = { [weak model, weak undoManager] in
            // A no-op registration: the document system reads it as "changed"
            // and does the rest. The edit itself is twig's to undo, through
            // the canvas view's `undo(_:)`, which answers Edit ▸ Undo before
            // the window would ask this manager.
            guard let model else { return }
            undoManager?.registerUndo(withTarget: model) { _ in }
        }
    }
}
