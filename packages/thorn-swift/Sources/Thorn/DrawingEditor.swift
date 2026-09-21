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
/// Under the tools, when there is something for it to say, the options: a
/// shape's words, for the selection when there is one and for the tool
/// when there is not — so far which ends of an arrow have a head
/// (`docs/proposals/shape-style.md` is the rest).
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
                VStack(spacing: 8) {
                    HStack(spacing: 8) {
                        ToolCluster { lockTile; hand; select }
                        ToolCluster { shapes }
                        ToolCluster { marks }
                        ToolCluster { layers; delete; undo; redo }
                    }
                    options
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
                VStack(spacing: 8) {
                    options
                    HStack(spacing: 8) {
                        ToolCluster {
                            ScrollView(.horizontal, showsIndicators: false) {
                                HStack(spacing: 2) { lockTile; hand; select; shapes; marks }
                            }
                        }
                        ToolCluster { undo; redo; more }
                    }
                }
                .padding(.horizontal, 10)
                .padding(.bottom, 6)
            }
    }
    #endif

    // MARK: The options

    /// The words the selection can take, or the tool's when nothing is
    /// selected; nothing at all when neither has any. The palette on one
    /// row — a colour, a background — and the stroke's words on the next.
    @ViewBuilder private var options: some View {
        let _ = state.revision
        let selected = !state.selection.isEmpty
        let color = selected ? !state.model.selectedColorable.isEmpty : state.tool.makesShape
        let fill = selected ? !state.model.selectedClosed.isEmpty : state.tool.makesClosedShape
        let dash = selected ? !state.model.selectedStroked.isEmpty : state.tool.makesStroke
        let heads = selected ? !state.model.selectedConnectors.isEmpty : state.tool == .arrow
        if color || fill {
            HStack(spacing: 8) {
                if color { ToolCluster { colorTiles } }
                if fill { ToolCluster { fillTiles } }
            }
        }
        if dash || heads {
            HStack(spacing: 8) {
                if dash { ToolCluster { weightTiles }; ToolCluster { dashTiles } }
                if heads { ToolCluster { headsTiles } }
            }
        }
    }

    /// How heavy the stroke is: thin, the template's, bold.
    @ViewBuilder private var weightTiles: some View {
        let current: Weight?? = state.selection.isEmpty ? .some(state.model.weight) : state.model.selectionWeight
        ForEach(WeightChoice.all, id: \.self) { choice in
            Button { state.model.setWeight(choice.weight) } label: {
                Label { Text(choice.name) } icon: { StrokeGlyph(width: choice.glyphWidth) }
            }
            .buttonStyle(ToolTile(on: current == .some(choice.weight)))
            .help(choice.name)
        }
    }

    /// The palette: the drawing's ink, then a swatch per hue in the
    /// colour the page's appearance draws it. For a selection it is what
    /// the shapes agree on, and lights nothing when they differ.
    @ViewBuilder private var colorTiles: some View {
        let current: Hue?? = state.selection.isEmpty ? .some(state.model.hue) : state.model.selectionColor
        let dark = state.model.appearance == .dark
        Button { state.model.setColor(nil) } label: {
            Label("Ink", systemImage: "circle")
        }
        .buttonStyle(ToolTile(on: current == .some(nil)))
        .help("The drawing's ink")
        ForEach(hues(), id: \.self) { hue in
            Button { state.model.setColor(hue) } label: {
                Label { Text(hue.name) } icon: { Swatch(hex: hueHex(hue: hue, dark: dark, tint: false)) }
            }
            .buttonStyle(ToolTile(on: current == .some(hue)))
            .help(hue.name)
        }
    }

    /// The backgrounds: none, then a tint per hue.
    @ViewBuilder private var fillTiles: some View {
        let current: Hue?? = state.selection.isEmpty ? .some(state.model.fill) : state.model.selectionFill
        let dark = state.model.appearance == .dark
        Button { state.model.setFill(nil) } label: {
            Label("No background", systemImage: "square.slash")
        }
        .buttonStyle(ToolTile(on: current == .some(nil)))
        .help("No background")
        ForEach(hues(), id: \.self) { hue in
            Button { state.model.setFill(hue) } label: {
                Label { Text("\(hue.name) background") } icon: { Swatch(hex: hueHex(hue: hue, dark: dark, tint: true), square: true) }
            }
            .buttonStyle(ToolTile(on: current == .some(hue)))
            .help("\(hue.name) background")
        }
    }

    /// How the stroke is broken: solid, dashed, dotted. For a selection it
    /// is what the stroked shapes agree on, and lights nothing when they
    /// differ.
    @ViewBuilder private var dashTiles: some View {
        let current: Dash?? = state.selection.isEmpty ? .some(state.model.dash) : state.model.selectionDash
        ForEach(DashChoice.all, id: \.self) { choice in
            Button { state.model.setDash(choice.dash) } label: {
                Label(choice.name, systemImage: choice.symbol)
            }
            .buttonStyle(ToolTile(on: current == .some(choice.dash)))
            .help(choice.name)
        }
    }

    /// Which ends have a head: none, the end, the start, both. For a
    /// selection it is what the connectors agree on, and lights nothing
    /// when they differ.
    @ViewBuilder private var headsTiles: some View {
        let current: Heads?? = state.selection.isEmpty ? .some(state.model.heads) : state.model.selectionHeads
        ForEach(Arrowheads.all, id: \.self) { choice in
            Button { state.model.setHeads(choice.heads) } label: {
                Label(choice.name, systemImage: choice.symbol)
            }
            .buttonStyle(ToolTile(on: current == .some(choice.heads)))
            .help(choice.name)
        }
    }

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

/// The choices for which ends of an arrow have a head, as the options
/// strip offers them: the values of `data-arrow`, and none.
enum Arrowheads: Hashable {
    case none, end, start, both

    static let all: [Arrowheads] = [.none, .end, .start, .both]

    var heads: Heads? {
        switch self {
        case .none: nil
        case .end: .end
        case .start: .start
        case .both: .both
        }
    }

    var name: String {
        switch self {
        case .none: "No arrowhead"
        case .end: "Arrowhead at the end"
        case .start: "Arrowhead at the start"
        case .both: "Arrowheads at both ends"
        }
    }

    var symbol: String {
        switch self {
        case .none: "minus"
        case .end: "arrow.right"
        case .start: "arrow.left"
        case .both: "arrow.left.and.right"
        }
    }
}

/// A swatch of the palette: a disc of a hue, or a square of its tint,
/// with a hairline so a pale one has an edge on a pale tile.
struct Swatch: View {
    let hex: String
    var square = false

    var body: some View {
        let color = Color(hex: hex)
        let side = ToolTile.glyph + 2
        if square {
            RoundedRectangle(cornerRadius: 3, style: .continuous)
                .fill(color)
                .overlay(RoundedRectangle(cornerRadius: 3, style: .continuous).strokeBorder(Color.primary.opacity(0.25)))
                .frame(width: side, height: side)
        } else {
            Circle()
                .fill(color)
                .overlay(Circle().strokeBorder(Color.primary.opacity(0.15)))
                .frame(width: side, height: side)
        }
    }
}

extension Color {
    /// A CSS hex colour, `#rgb` or `#rrggbb`, as the template spells them.
    init(hex: String) {
        var digits = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        if digits.count == 3 { digits = digits.map { "\($0)\($0)" }.joined() }
        let value = UInt64(digits, radix: 16) ?? 0
        self.init(
            red: Double((value >> 16) & 0xff) / 255,
            green: Double((value >> 8) & 0xff) / 255,
            blue: Double(value & 0xff) / 255)
    }
}

extension Hue {
    /// The hue's name, capitalised, for a tooltip.
    var name: String {
        switch self {
        case .red: "Red"
        case .orange: "Orange"
        case .yellow: "Yellow"
        case .green: "Green"
        case .blue: "Blue"
        case .violet: "Violet"
        case .pink: "Pink"
        case .grey: "Grey"
        }
    }
}

/// The choices for how heavy a stroke is, as the options strip offers
/// them: the values of `data-weight`, and the template's width between.
enum WeightChoice: Hashable {
    case thin, regular, bold

    static let all: [WeightChoice] = [.thin, .regular, .bold]

    var weight: Weight? {
        switch self {
        case .thin: .thin
        case .regular: nil
        case .bold: .bold
        }
    }

    var name: String {
        switch self {
        case .thin: "Thin"
        case .regular: "Regular"
        case .bold: "Bold"
        }
    }

    /// The line the tile shows, in points.
    var glyphWidth: CGFloat {
        switch self {
        case .thin: 1
        case .regular: 2
        case .bold: 4
        }
    }
}

/// A short horizontal line of a given width: the weight tile's glyph,
/// drawn in the tile's foreground so it lights with it.
struct StrokeGlyph: View {
    let width: CGFloat

    var body: some View {
        Capsule()
            .frame(width: ToolTile.glyph + 2, height: width)
    }
}

/// The choices for how a stroke is broken, as the options strip offers
/// them: the values of `data-dash`, and solid.
enum DashChoice: Hashable {
    case solid, dashed, dotted

    static let all: [DashChoice] = [.solid, .dashed, .dotted]

    var dash: Dash? {
        switch self {
        case .solid: nil
        case .dashed: .dashed
        case .dotted: .dotted
        }
    }

    var name: String {
        switch self {
        case .solid: "Solid"
        case .dashed: "Dashed"
        case .dotted: "Dotted"
        }
    }

    var symbol: String {
        switch self {
        case .solid: "square"
        case .dashed: "square.dashed"
        case .dotted: "circle.dotted"
        }
    }
}

/// The bit of the model SwiftUI watches: the tool, the lock, the
/// selection, the zoom, and a count that moves whenever the options strip
/// may have something different to show.
final class EditorState: ObservableObject {
    let model: CanvasModel
    @Published var tool: Tool
    @Published var locked: Bool
    @Published var selection: [String]
    @Published var zoom: CGFloat
    @Published var revision = 0

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
        model.onOptionsChange = { [weak self] in self?.revision += 1 }
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
