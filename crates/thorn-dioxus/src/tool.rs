//! The toolset, Excalidraw's shape: the tools in toolbar order, each with a
//! name, an icon and the keys that pick it. One table, so the toolbar and
//! the canvas's keys cannot disagree about which key is which tool.
//!
//! The lock — keeping a create tool after the shape it made — is not a tool
//! but a flag on the canvas, [`Canvas::locked`](crate::Canvas::locked), with
//! a key here.

use crate::icons;

/// What the next pointer-down does.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum Tool {
    /// Pan the view by dragging.
    Hand,
    /// Select, move by dragging, resize by a handle.
    #[default]
    Select,
    Rect,
    Diamond,
    Ellipse,
    Arrow,
    Line,
    /// Freehand ink.
    Draw,
    /// Place a label, typed into a field the canvas opens at the click.
    Text,
    /// A box with a label in it.
    Note,
    /// Delete what is dragged over.
    Eraser,
}

impl Tool {
    /// The tools in toolbar order.
    pub const ALL: [Tool; 11] = [
        Tool::Hand,
        Tool::Select,
        Tool::Rect,
        Tool::Diamond,
        Tool::Ellipse,
        Tool::Arrow,
        Tool::Line,
        Tool::Draw,
        Tool::Text,
        Tool::Note,
        Tool::Eraser,
    ];

    /// The key that toggles the lock.
    pub const LOCK_KEY: char = 'q';

    pub fn name(self) -> &'static str {
        match self {
            Tool::Hand => "Hand",
            Tool::Select => "Select",
            Tool::Rect => "Rectangle",
            Tool::Diamond => "Diamond",
            Tool::Ellipse => "Ellipse",
            Tool::Arrow => "Arrow",
            Tool::Line => "Line",
            Tool::Draw => "Draw",
            Tool::Text => "Text",
            Tool::Note => "Note",
            Tool::Eraser => "Eraser",
        }
    }

    /// The icon its tile shows, on [`unit::Icon`]'s grid.
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Hand => icons::HAND,
            Tool::Select => icons::POINTER,
            Tool::Rect => icons::SQUARE,
            Tool::Diamond => icons::DIAMOND,
            Tool::Ellipse => icons::CIRCLE,
            Tool::Arrow => icons::ARROW,
            Tool::Line => icons::LINE,
            Tool::Draw => icons::PENCIL,
            Tool::Text => icons::TYPE,
            Tool::Note => icons::NOTE,
            Tool::Eraser => icons::ERASER,
        }
    }

    /// The keys that pick this tool, a letter and a digit, in Excalidraw's
    /// order: the digit is the tool's place in the toolbar, with `0` for
    /// the tenth.
    pub fn keys(self) -> &'static [char] {
        match self {
            Tool::Hand => &['h'],
            Tool::Select => &['v', '1'],
            Tool::Rect => &['r', '2'],
            Tool::Diamond => &['d', '3'],
            Tool::Ellipse => &['o', '4'],
            Tool::Arrow => &['a', '5'],
            Tool::Line => &['l', '6'],
            Tool::Draw => &['p', '7'],
            Tool::Text => &['t', '8'],
            Tool::Note => &['n', '9'],
            Tool::Eraser => &['e', '0'],
        }
    }

    /// The tool a key picks, or `None` for a key that picks none.
    pub fn for_key(key: char) -> Option<Tool> {
        let key = key.to_ascii_lowercase();
        Tool::ALL.into_iter().find(|t| t.keys().contains(&key))
    }

    /// Whether one use of this tool makes one shape, after which the tool
    /// falls back to select — unless the canvas is locked. The pen is kept,
    /// as Excalidraw's is: a word is written in several strokes.
    pub fn is_one_shot(self) -> bool {
        matches!(
            self,
            Tool::Rect
                | Tool::Diamond
                | Tool::Ellipse
                | Tool::Arrow
                | Tool::Line
                | Tool::Text
                | Tool::Note
        )
    }

    /// Whether the shape this tool makes is drawn as a stroke, so a dash
    /// or a weight picked with nothing selected is its to take. A note's
    /// frame is.
    pub fn makes_stroke(self) -> bool {
        matches!(
            self,
            Tool::Rect | Tool::Diamond | Tool::Ellipse | Tool::Arrow | Tool::Line | Tool::Note
        )
    }

    /// Whether this tool makes a shape a colour lands on: every create
    /// tool does.
    pub fn makes_shape(self) -> bool {
        self.is_one_shot() || self == Tool::Draw
    }

    /// Whether this tool makes a box, so a corner picked with nothing
    /// selected is its to take. A note's frame is one.
    pub fn makes_box(self) -> bool {
        matches!(self, Tool::Rect | Tool::Note)
    }

    /// Whether the shape this tool makes is closed, so a background picked
    /// with nothing selected is its to take. A note's frame is.
    pub fn makes_closed_shape(self) -> bool {
        matches!(
            self,
            Tool::Rect | Tool::Diamond | Tool::Ellipse | Tool::Note
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_picks_one_tool_and_the_digits_are_the_toolbar_order() {
        let mut seen = std::collections::HashSet::new();
        for tool in Tool::ALL {
            for &k in tool.keys() {
                assert!(seen.insert(k), "{k} picks two tools");
                assert_eq!(Tool::for_key(k), Some(tool));
            }
        }
        assert!(!seen.contains(&Tool::LOCK_KEY));
        // Select is the first after the hand, as in Excalidraw.
        for (i, tool) in Tool::ALL.iter().skip(1).enumerate() {
            let digit = char::from_digit(((i + 1) % 10) as u32, 10).unwrap();
            assert!(tool.keys().contains(&digit), "{tool:?} has no {digit}");
        }
        assert_eq!(Tool::for_key('R'), Some(Tool::Rect));
    }
}
