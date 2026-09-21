// A SwiftUI editor: the canvas with its tools floating over it — a strip at
// the top on the Mac and an iPad, a bar along the bottom on a phone. A host
// that wants its own chrome uses `DrawingCanvasView` and `CanvasModel`
// directly; this is the whole thing for one that does not.

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

/// The canvas with its tools floating over it, in Excalidraw's shape: the
/// lock, then every tool in `Tool.all` (each with its keys in its tooltip);
/// a menu of the layering and grouping commands; delete; undo and redo;
/// and the zoom — out, the percentage (which fits the picture again), in.
///
/// Where the chrome sits is the width's: on the Mac, and on an iPad where
/// the whole strip fits — a strip of capsules top-centre and the zoom
/// bottom-trailing, the canvas the whole height under both; narrower than
/// that — a phone, an iPad's detail column beside a sidebar — one bar
/// along the bottom, in thumb's reach, the tools scrolling sideways past a
/// fixed undo/redo/more. Measured, not the size class: a split view's
/// column is "regular" and 540 pt wide. A tile is 32 pt on the Mac and 44
/// pt on iOS; the one that is on is a solid accent tile with a white
/// glyph, and every other is monochrome, so the row reads as a palette
/// with one thing chosen rather than as a line of blue links.
///
/// On iOS the canvas ignores the keyboard's safe area: the field for a
/// label sits at a view point of the picture, and a canvas that shrank for
/// the keyboard would slide the picture out from under it. The canvas view
/// pans the picture up itself when the keyboard would cover the field.
public struct DrawingEditor: View {
    @ObservedObject private var state: EditorState

    public init(document: DrawingDocument) {
        self.init(model: CanvasModel(document: document))
    }

    /// Over a model the host owns — so it can listen to `onDocumentChange`
    /// and save, or pick the tool from outside the toolbar.
    public init(model: CanvasModel) {
        state = EditorState(model: model)
    }

    public var body: some View {
        #if os(macOS)
        strip
        #else
        GeometryReader { geometry in
            if geometry.size.width >= Self.stripWidth { strip } else { bar }
        }
        .ignoresSafeArea(.keyboard)
        #endif
    }

    #if !os(macOS)
    /// What the strip takes across: sixteen tiles in four capsules, and
    /// the padding around them.
    static let stripWidth: CGFloat = 16 * (ToolTile.side + 2) + 4 * 8 + 3 * 8 + 2 * 10
    #endif

    /// The tools top-centre and the zoom bottom-trailing.
    private var strip: some View {
        DrawingCanvas(model: state.model)
            .overlay(alignment: .top) {
                HStack(spacing: 8) {
                    ToolCluster { lockTile; hand; select }
                    ToolCluster { shapes }
                    ToolCluster { marks }
                    ToolCluster { layers; delete; undo; redo }
                }
                .padding(10)
            }
            .overlay(alignment: .bottomTrailing) {
                ToolCluster { zoomOut; zoomLabel; zoomIn }
                    .padding(10)
            }
    }

    #if !os(macOS)
    /// One bar along the bottom, the tools scrolling past undo, redo and
    /// more.
    private var bar: some View {
        DrawingCanvas(model: state.model)
            .overlay(alignment: .bottom) {
                HStack(spacing: 8) {
                    ToolCluster {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 2) { lockTile; hand; select; shapes; marks }
                        }
                    }
                    ToolCluster { undo; redo; more }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 6)
            }
    }
    #endif

    // MARK: The tiles

    @ViewBuilder private var hand: some View { toolTile(.hand) }
    @ViewBuilder private var select: some View { toolTile(.select) }
    @ViewBuilder private var shapes: some View {
        ForEach([Tool.rect, .diamond, .ellipse, .arrow, .line], id: \.self) { toolTile($0) }
    }
    @ViewBuilder private var marks: some View {
        ForEach([Tool.draw, .text(), .note, .eraser], id: \.self) { toolTile($0) }
    }

    private func toolTile(_ tool: Tool) -> some View {
        let keys = tool.keys.map { $0.uppercased() }.joined(separator: " or ")
        return Button { state.setTool(tool) } label: {
            Label(tool.name, systemImage: tool.symbol)
        }
        .buttonStyle(ToolTile(on: state.tool == tool))
        .help("\(tool.name) — \(keys)")
    }

    /// The lock is a toggle on the tools, not a tool: it lights the same
    /// way, and stays lit while every create tool is kept.
    private var lockTile: some View {
        Button { state.model.locked.toggle() } label: {
            Label("Keep tool", systemImage: state.locked ? "lock.fill" : "lock.open")
        }
        .buttonStyle(ToolTile(on: state.locked))
        .help("Keep the tool after drawing — \(Tool.lockKey.uppercased())")
    }

    private var layers: some View {
        Menu {
            layerCommands
        } label: {
            Label("Layers and groups", systemImage: "square.3.layers.3d")
        }
        .menuStyle(.button)
        .buttonStyle(ToolTile(on: false))
        .menuIndicator(.hidden)
        .fixedSize()
        .help("Layers and groups")
        .disabled(state.selection.isEmpty)
    }

    @ViewBuilder private var layerCommands: some View {
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
    }

    private var delete: some View {
        Button { state.model.deleteSelection() } label: { Label("Delete", systemImage: "trash") }
            .buttonStyle(ToolTile(on: false))
            .help("Delete")
            .disabled(state.selection.isEmpty)
    }

    /// Undo and redo take ⌘Z and ⇧⌘Z on iOS, where there is no Edit menu
    /// to hand them down the responder chain; on the Mac the app's menu
    /// does, through `DrawingCanvasView.undo(_:)`.
    private var undo: some View {
        Button { state.model.undo() } label: { Label("Undo", systemImage: "arrow.uturn.backward") }
            .buttonStyle(ToolTile(on: false))
            .help("Undo")
            .iOSShortcut("z", modifiers: .command)
    }

    private var redo: some View {
        Button { state.model.redo() } label: { Label("Redo", systemImage: "arrow.uturn.forward") }
            .buttonStyle(ToolTile(on: false))
            .help("Redo")
            .iOSShortcut("z", modifiers: [.command, .shift])
    }

    private var zoomOut: some View {
        Button { state.model.zoomOut() } label: { Label("Zoom out", systemImage: "minus.magnifyingglass") }
            .buttonStyle(ToolTile(on: false))
            .keyboardShortcut("-", modifiers: .command).help("Zoom out — ⌘−")
            .disabled(state.zoom <= CanvasModel.zoomRange.lowerBound)
    }

    private var zoomIn: some View {
        Button { state.model.zoomIn() } label: { Label("Zoom in", systemImage: "plus.magnifyingglass") }
            .buttonStyle(ToolTile(on: false))
            .keyboardShortcut("=", modifiers: .command).help("Zoom in — ⌘+")
            .disabled(state.zoom >= CanvasModel.zoomRange.upperBound)
    }

    private var zoomLabel: some View {
        Button { state.model.zoomToFit() } label: {
            Text("\(Int((state.zoom * 100).rounded()))%")
                .monospacedDigit()
                .font(.system(size: 12, weight: .medium))
                .frame(minWidth: 44)
        }
        .buttonStyle(ToolTile(on: false, wide: true))
        .keyboardShortcut("0", modifiers: .command).help("Fit the drawing — ⌘0")
    }

    /// On a phone the layering and grouping commands, delete, and the fit
    /// live behind one tile; the zoom itself is the pinch.
    private var more: some View {
        Menu {
            Section { layerCommands }
            Section {
                Button("Delete", role: .destructive) { state.model.deleteSelection() }
                    .disabled(state.selection.isEmpty)
            }
            Section {
                Button("Fit the drawing") { state.model.zoomToFit() }
            }
        } label: {
            Label("More", systemImage: "ellipsis")
        }
        .menuStyle(.button)
        .buttonStyle(ToolTile(on: false))
        .menuIndicator(.hidden)
        .fixedSize()
    }
}

/// One tile of the palette: a fixed square, the glyph alone, monochrome
/// until it is on, when it is a solid accent tile with a white glyph. The
/// Mac shows a faint fill under the pointer; disabled fades the glyph.
struct ToolTile: ButtonStyle {
    let on: Bool
    var wide = false
    @Environment(\.isEnabled) private var isEnabled
    @State private var hovering = false

    #if os(macOS)
    static let side: CGFloat = 32
    static let glyph: CGFloat = 14
    #else
    static let side: CGFloat = 44
    static let glyph: CGFloat = 19
    #endif
    static let radius: CGFloat = 8

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .labelStyle(.iconOnly)
            .font(.system(size: Self.glyph, weight: .medium))
            .frame(width: wide ? nil : Self.side, height: Self.side)
            .padding(.horizontal, wide ? 6 : 0)
            .foregroundStyle(on ? AnyShapeStyle(.white) : AnyShapeStyle(.primary))
            .background(fill(pressed: configuration.isPressed), in: RoundedRectangle(cornerRadius: Self.radius, style: .continuous))
            .contentShape(RoundedRectangle(cornerRadius: Self.radius, style: .continuous))
            .opacity(isEnabled ? 1 : 0.35)
            .onHover { hovering = $0 }
            .animation(.easeOut(duration: 0.12), value: on)
    }

    private func fill(pressed: Bool) -> Color {
        if on { return pressed ? Color.accentColor.opacity(0.8) : Color.accentColor }
        if pressed { return Color.primary.opacity(0.16) }
        if hovering { return Color.primary.opacity(0.08) }
        return .clear
    }
}

/// A capsule of tiles: the material behind a cluster, and its hairline.
struct ToolCluster<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        HStack(spacing: 2) { content }
            .padding(4)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: ToolTile.radius + 4, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: ToolTile.radius + 4, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.08))
            )
            .shadow(color: .black.opacity(0.08), radius: 6, y: 2)
    }
}

/// The bit of the model SwiftUI watches: the tool, the lock, the
/// selection and the zoom.
final class EditorState: ObservableObject {
    let model: CanvasModel
    @Published var tool: Tool
    @Published var locked: Bool
    @Published var selection: [String]
    @Published var zoom: CGFloat

    init(model: CanvasModel) {
        self.model = model
        tool = model.tool
        locked = model.locked
        selection = model.selection
        zoom = model.zoom
        model.onSelectionChange = { [weak self] ids in self?.selection = ids }
        model.onToolChange = { [weak self] tool in self?.tool = tool }
        model.onLockChange = { [weak self] locked in self?.locked = locked }
        model.onZoomChange = { [weak self] zoom in self?.zoom = zoom }
    }

    func setTool(_ tool: Tool) { model.tool = tool }
}

extension View {
    /// A keyboard shortcut on iOS only.
    @ViewBuilder func iOSShortcut(_ key: KeyEquivalent, modifiers: EventModifiers) -> some View {
        #if os(macOS)
        self
        #else
        keyboardShortcut(key, modifiers: modifiers)
        #endif
    }
}

#if canImport(AppKit)
typealias PlatformViewRepresentable = NSViewRepresentable
#else
typealias PlatformViewRepresentable = UIViewRepresentable
#endif
#endif
