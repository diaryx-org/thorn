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

/// The canvas with a toolbar: select, rectangle, ellipse, line, label;
/// forward and back; group and ungroup; delete; undo and redo.
public struct DrawingEditor: View {
    @ObservedObject private var state: EditorState

    public init(document: DrawingDocument) {
        state = EditorState(model: CanvasModel(document: document))
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                toolButton("Select", "arrow.up.left", .select)
                toolButton("Rectangle", "rectangle", .rect)
                toolButton("Ellipse", "oval", .ellipse)
                toolButton("Line", "line.diagonal", .line)
                toolButton("Label", "textformat", .text())
                Divider().frame(height: 16)
                Button { state.model.reorderSelection(.forward) } label: { Image(systemName: "square.2.layers.3d.top.filled") }
                    .help("Bring forward").disabled(state.selection.count != 1)
                Button { state.model.reorderSelection(.backward) } label: { Image(systemName: "square.2.layers.3d.bottom.filled") }
                    .help("Send backward").disabled(state.selection.count != 1)
                Button { state.model.groupSelection() } label: { Image(systemName: "rectangle.3.group") }
                    .help("Group").keyboardShortcut("g").disabled(!state.model.canGroup)
                Button { state.model.ungroupSelection() } label: { Image(systemName: "rectangle.3.group.bubble") }
                    .help("Ungroup").keyboardShortcut("g", modifiers: [.command, .shift]).disabled(!state.model.canUngroup)
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

    private func toolButton(_ name: String, _ symbol: String, _ tool: Tool) -> some View {
        Button { state.setTool(tool) } label: {
            Image(systemName: symbol)
                .padding(4)
                .background(state.tool == tool ? Color.accentColor.opacity(0.25) : Color.clear)
                .cornerRadius(4)
        }
        .help(name)
    }
}

/// The bit of the model SwiftUI watches: the tool and the selection.
final class EditorState: ObservableObject {
    let model: CanvasModel
    @Published var tool: Tool
    @Published var selection: [String]

    init(model: CanvasModel) {
        self.model = model
        tool = model.tool
        selection = model.selection
        model.onSelectionChange = { [weak self] ids in self?.selection = ids }
        model.onToolChange = { [weak self] tool in self?.tool = tool }
    }

    func setTool(_ tool: Tool) { model.tool = tool }
}

#if canImport(AppKit)
typealias PlatformViewRepresentable = NSViewRepresentable
#else
typealias PlatformViewRepresentable = UIViewRepresentable
#endif
#endif
