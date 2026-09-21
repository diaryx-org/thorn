// Open a drawing in a window. With no argument, a fresh page.

import AppKit
import Thorn
import SwiftUI

let path = CommandLine.arguments.dropFirst().first
let document = try path.map { try DrawingDocument(contentsOf: URL(fileURLWithPath: $0)) }
    ?? DrawingDocument.fresh()

final class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!

    func applicationDidFinishLaunching(_ notification: Notification) {
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 760, height: 520),
            styleMask: [.titled, .closable, .resizable, .miniaturizable],
            backing: .buffered, defer: false)
        window.title = path.map { ($0 as NSString).lastPathComponent } ?? "Untitled drawing"
        window.contentView = NSHostingView(rootView: DrawingEditor(document: document))
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationWillTerminate(_ notification: Notification) {
        // What the user drew is printed, since this host has no Save.
        if let path { try? document.source.write(toFile: path, atomically: true, encoding: .utf8) }
        else { print(document.source) }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
