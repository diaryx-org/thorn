//  ThornApp.swift
//
//  Thorn, the drawing app: a window per file, one `ThornDocument` behind each.
//  `DocumentGroup` is the whole of the file story — the Open panel at launch,
//  File ▸ New / Open / Save / Save As / Duplicate / Rename / Revert, autosave
//  and Versions, the recents list, the proxy icon in the title bar, Finder's
//  "Open With" once `project.yml` has declared the type; on iOS the document
//  browser and the Files app. The editor's own commands — the tools' keys, the
//  layering menu, ⌘+/⌘−/⌘0 — are the package's, in `DrawingEditor`; Edit ▸
//  Undo and Redo reach the canvas down the responder chain.

import SwiftUI
import Thorn

@main
struct ThornApp: App {
    var body: some Scene {
        DocumentGroup(newDocument: { ThornDocument() }) { file in
            ContentView(document: file.document)
                .sized()
        }
        .commands {
            ThornAppCommands()
        }
    }
}

private extension View {
    func sized() -> some View {
        #if os(macOS)
        frame(minWidth: 480, idealWidth: 760, minHeight: 320, idealHeight: 520)
        #else
        self
        #endif
    }
}

/// The app's own menu items, beside what `DocumentGroup` provides.
struct ThornAppCommands: Commands {
    var body: some Commands {
        #if os(macOS)
        CommandGroup(after: .help) {
            Divider()
            Link("The Diaryx drawing profile",
                 destination: URL(string: "https://github.com/diaryx-org/thorn/blob/main/docs/profile.md")!)
        }
        #endif
    }
}
