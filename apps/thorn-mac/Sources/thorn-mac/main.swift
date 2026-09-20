// Open a drawing in a window. With no argument, a fresh page.

import AppKit
import Thorn
import SwiftUI

let fresh = """
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 640 400" width="640" height="400" data-diaryx-drawing="1">
      <style>
        rect, ellipse { fill: none; stroke: #222; stroke-width: 2 }
        line { stroke: #222; stroke-width: 2 }
        text { font: 16px sans-serif }
      </style>
    </svg>

    """

let path = CommandLine.arguments.dropFirst().first
let source = path.flatMap { try? String(contentsOfFile: $0, encoding: .utf8) } ?? fresh
let document = try DrawingDocument(source: source)

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
