// The toolset, Excalidraw's shape: the tools in toolbar order, each with a
// name, a symbol and the keys that pick it. One table, so the toolbar and
// the canvas's key handler cannot disagree about which key is which tool.
//
// The lock — keeping a create tool after the shape it made — is not a tool
// but a flag on the model, `CanvasModel.locked`, with a key here.

extension Tool {
    /// The tools in toolbar order.
    public static let all: [Tool] = [.hand, .select, .rect, .diamond, .ellipse, .arrow, .line, .draw, .text(), .note, .eraser]

    /// The key that toggles `CanvasModel.locked`.
    public static let lockKey: Character = "q"

    public var name: String {
        switch self {
        case .hand: "Hand"
        case .select: "Select"
        case .rect: "Rectangle"
        case .diamond: "Diamond"
        case .ellipse: "Ellipse"
        case .arrow: "Arrow"
        case .line: "Line"
        case .draw: "Draw"
        case .text: "Text"
        case .note: "Note"
        case .eraser: "Eraser"
        }
    }

    /// An SF Symbol.
    public var symbol: String {
        switch self {
        case .hand: "hand.raised"
        case .select: "cursorarrow"
        case .rect: "rectangle"
        case .diamond: "diamond"
        case .ellipse: "oval"
        case .arrow: "arrow.up.right"
        case .line: "line.diagonal"
        case .draw: "pencil"
        case .text: "textformat"
        case .note: "note.text"
        case .eraser: "eraser"
        }
    }

    /// The keys that pick this tool, a letter and a digit, in Excalidraw's
    /// order: the digit is the tool's place in the toolbar, with `0` for
    /// the tenth.
    public var keys: [Character] {
        switch self {
        case .hand: ["h"]
        case .select: ["v", "1"]
        case .rect: ["r", "2"]
        case .diamond: ["d", "3"]
        case .ellipse: ["o", "4"]
        case .arrow: ["a", "5"]
        case .line: ["l", "6"]
        case .draw: ["p", "7"]
        case .text: ["t", "8"]
        case .note: ["n", "9"]
        case .eraser: ["e", "0"]
        }
    }

    /// The tool a key picks, or `nil` for a key that picks none.
    public static func forKey(_ key: Character) -> Tool? {
        all.first { $0.keys.contains(Character(key.lowercased())) }
    }

    /// Whether the core has the gesture this tool makes. Every tool does
    /// now; a tool added ahead of its gesture sits in the toolbar disabled
    /// until the core has it.
    public var isAvailable: Bool {
        switch self {
        case .hand, .select, .rect, .diamond, .ellipse, .arrow, .line, .draw, .text, .note, .eraser: true
        }
    }

    /// Whether one use of this tool makes one shape, after which the tool
    /// falls back to select — unless the model is locked.
    var isOneShot: Bool {
        switch self {
        case .hand, .select, .eraser: false
        case .rect, .diamond, .ellipse, .arrow, .line, .draw, .text, .note: true
        }
    }

    /// Whether the shape this tool makes is drawn as a stroke, so a dash
    /// picked with nothing selected is its to take. A note's frame is.
    var makesStroke: Bool {
        switch self {
        case .rect, .diamond, .ellipse, .arrow, .line, .note: true
        case .hand, .select, .eraser, .draw, .text: false
        }
    }

    /// Whether this tool makes a shape a colour lands on: every create
    /// tool does.
    var makesShape: Bool { isOneShot }

    /// Whether the shape this tool makes is closed, so a background picked
    /// with nothing selected is its to take. A note's frame is.
    var makesClosedShape: Bool {
        switch self {
        case .rect, .diamond, .ellipse, .note: true
        case .hand, .select, .eraser, .draw, .text, .arrow, .line: false
        }
    }
}
