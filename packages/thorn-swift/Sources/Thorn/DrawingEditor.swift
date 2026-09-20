// A SwiftUI editor: the canvas with a toolbar above it, the same on both
// platforms. A host that wants its own chrome uses `DrawingCanvasView` and
// `CanvasModel` directly; this is the whole thing for one that does not.

#if canImport(SwiftUI)
import SwiftUI
import ThornFFI

/// The canvas as a SwiftUI view.
public struct DrawingCanvas: PlatformViewRepresentable {
    public let model: CanvasModel

    public init(model: CanvasModel) { self.model = model }

    #if canImport(AppKit)
    public func makeNSView(context: Context) -> DrawingCanvasView { DrawingCanvasView(model: model) }
    public func updateNSView(_ view: DrawingCanvasView, context: Context) { view.needsDisplay = true }
    #else
    public func makeUIView(context: Context) -> DrawingCanvasView { DrawingCanvasView(model: model) }
    public func updateUIView(_ view: DrawingCanvasView, context: Context) { view.setNeedsDisplay() }
    #endif
}

/// The canvas with a toolbar in Excalidraw's shape: the lock, then every
/// tool in `Tool.all` (each with its keys in its tooltip, and disabled
/// until the core has its gesture); a menu of the layering and grouping
/// commands; delete; undo and redo.
public struct DrawingEditor: View {
    @ObservedObject private var state: EditorState

    public init(document: DrawingDocument) {
        state = EditorState(model: CanvasModel(document: document))
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Button { state.model.locked.toggle() } label: {
                    Image(systemName: state.locked ? "lock.fill" : "lock.open")
                        .padding(4)
                        .background(state.locked ? Color.accentColor.opacity(0.25) : Color.clear)
                        .cornerRadius(4)
                }
                .help("Keep the tool after drawing — \(Tool.lockKey.uppercased())")
                Divider().frame(height: 16)
                ForEach(Tool.all, id: \.self) { tool in toolButton(tool) }
                Divider().frame(height: 16)
                Menu {
                    Button("Bring to front") { state.model.reorderSelection(.toFront) }
                        .keyboardShortcut("]", modifiers: [.command, .shift])
                    Button("Bring forward") { state.model.reorderSelection(.forward) }
                        .keyboardShortcut("]", modifiers: .command)
                    Button("Send backward") { state.model.reorderSelection(.backward) }
                        .keyboardShortcut("[", modifiers: .command)
                    Button("Send to back") { state.model.reorderSelection(.toBack) }
                        .keyboardShortcut("[", modifiers: [.command, .shift])
                    Divider()
                    Button("Group") { state.model.groupSelection() }
                        .keyboardShortcut("g").disabled(!state.model.canGroup)
                    Button("Ungroup") { state.model.ungroupSelection() }
                        .keyboardShortcut("g", modifiers: [.command, .shift]).disabled(!state.model.canUngroup)
                } label: {
                    Image(systemName: "square.3.layers.3d")
                }
                .menuIndicator(.hidden)
                .fixedSize()
                .help("Layers and groups")
                .disabled(state.selection.isEmpty)
                Button { state.model.deleteSelection() } label: { Image(systemName: "trash") }
                    .help("Delete").disabled(state.selection.isEmpty)
                Divider().frame(height: 16)
                Button { state.model.undo() } label: { Image(systemName: "arrow.uturn.backward") }.help("Undo")
                Button { state.model.redo() } label: { Image(systemName: "arrow.uturn.forward") }.help("Redo")
                Spacer()
            }
            .padding(8)
            .buttonStyle(.borderless)
            DrawingCanvas(model: state.model)
        }
    }

    private func toolButton(_ tool: Tool) -> some View {
        let keys = tool.keys.map { $0.uppercased() }.joined(separator: " or ")
        return Button { state.setTool(tool) } label: {
            Image(systemName: tool.symbol)
                .padding(4)
                .background(state.tool == tool ? Color.accentColor.opacity(0.25) : Color.clear)
                .cornerRadius(4)
        }
        .help(tool.isAvailable ? "\(tool.name) — \(keys)" : "\(tool.name) — not yet")
        .disabled(!tool.isAvailable)
    }
}

/// The bit of the model SwiftUI watches: the tool, the lock and the
/// selection.
final class EditorState: ObservableObject {
    let model: CanvasModel
    @Published var tool: Tool
    @Published var locked: Bool
    @Published var selection: [String]

    init(model: CanvasModel) {
        self.model = model
        tool = model.tool
        locked = model.locked
        selection = model.selection
        model.onSelectionChange = { [weak self] ids in self?.selection = ids }
        model.onToolChange = { [weak self] tool in self?.tool = tool }
        model.onLockChange = { [weak self] locked in self?.locked = locked }
    }

    func setTool(_ tool: Tool) { model.tool = tool }
}

#if canImport(AppKit)
typealias PlatformViewRepresentable = NSViewRepresentable
#else
typealias PlatformViewRepresentable = UIViewRepresentable
#endif
#endif
