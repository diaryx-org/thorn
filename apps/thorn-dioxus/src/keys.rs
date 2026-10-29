//! winit's keys as Dioxus's: what the window hears, in the words unit and
//! the canvas take. leaf's conversion, for the same reason: under
//! `Keys::Host` the app reads keys from the window itself.

use std::str::FromStr;

use dioxus::html::{Key, Modifiers, NamedKey};
use dioxus_native::winit::event::KeyEvent;
use dioxus_native::winit::keyboard::{self, ModifiersState};

/// A key pressed, with the modifiers held; `None` for a release.
pub fn pressed(event: &KeyEvent, mods: ModifiersState) -> Option<(Key, Modifiers)> {
    if !event.state.is_pressed() {
        return None;
    }
    Some((key(&event.logical_key), modifiers(mods)))
}

/// winit's named keys carry the W3C names `keyboard-types` parses, so the
/// `Debug` spelling is the conversion; a character is itself.
fn key(k: &keyboard::Key) -> Key {
    match k {
        keyboard::Key::Character(s) => Key::Character(s.to_string()),
        keyboard::Key::Named(n) => {
            Key::from_str(&format!("{n:?}")).unwrap_or(Key::Named(NamedKey::Unidentified))
        }
        _ => Key::Named(NamedKey::Unidentified),
    }
}

/// winit's `meta` is ⌘, which `keyboard-types` calls `META`.
fn modifiers(m: ModifiersState) -> Modifiers {
    let mut out = Modifiers::empty();
    out.set(Modifiers::SHIFT, m.shift_key());
    out.set(Modifiers::CONTROL, m.control_key());
    out.set(Modifiers::ALT, m.alt_key());
    out.set(Modifiers::META, m.meta_key());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus_native::winit::keyboard::NamedKey as WinitNamed;

    #[test]
    fn named_keys_convert_by_name() {
        assert_eq!(
            key(&keyboard::Key::Named(WinitNamed::Escape)),
            Key::Named(NamedKey::Escape)
        );
        assert_eq!(
            key(&keyboard::Key::Named(WinitNamed::Backspace)),
            Key::Named(NamedKey::Backspace)
        );
        assert_eq!(
            key(&keyboard::Key::Character("r".into())),
            Key::Character("r".into())
        );
    }
}
