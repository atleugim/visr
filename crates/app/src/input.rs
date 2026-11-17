//! GPUI keystrokes to the bytes the terminal program expects.

use gpui::Keystroke;
use term::keys::{Key, Mods, encode};

/// `None` when the keystroke isn't for the key-down path: platform shortcuts (cmd on macOS),
/// a modifier on its own, or text, which the input handler types so IME and dead keys work.
pub(crate) fn keystroke_bytes(keystroke: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
  keystroke_bytes_with(keystroke, app_cursor, cfg!(windows))
}

/// `altgr_is_ctrl_alt`: this platform reports AltGr as ctrl and alt together (Windows).
fn keystroke_bytes_with(
  keystroke: &Keystroke,
  app_cursor: bool,
  altgr_is_ctrl_alt: bool,
) -> Option<Vec<u8>> {
  let modifiers = keystroke.modifiers;
  if modifiers.platform {
    return None;
  }
  let mods = Mods {
    ctrl: modifiers.control,
    alt: modifiers.alt,
    shift: modifiers.shift,
  };
  if let Some(key) = named_key(&keystroke.key) {
    return Some(encode(key, mods, app_cursor));
  }
  if is_composed(keystroke, altgr_is_ctrl_alt) {
    return None;
  }
  let c = match keystroke.key.as_str() {
    "space" => ' ',
    key => {
      let mut chars = key.chars();
      match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => return None,
      }
    }
  };
  Some(encode(Key::Char(c), mods, app_cursor))
}

/// Whether the keystroke typed text, which the input handler takes, rather than a key for the
/// program. Composed text wins over meta, so option-2 types @ on Latin American Mac layouts. With
/// ctrl held it is a control key, except for AltGr on a platform that reports it as ctrl+alt:
/// there a character the key doesn't already name (ignoring the case shift makes) is text.
fn is_composed(keystroke: &Keystroke, altgr_is_ctrl_alt: bool) -> bool {
  let modifiers = keystroke.modifiers;
  keystroke.key_char.as_deref().is_some_and(|typed| {
    if !modifiers.control {
      return true;
    }
    altgr_is_ctrl_alt && modifiers.alt && !typed.eq_ignore_ascii_case(&keystroke.key)
  })
}

/// What the keymap matches against: the key's name and exactly which modifiers were held.
pub(crate) fn key_press(keystroke: &Keystroke) -> config::KeyPress {
  let modifiers = keystroke.modifiers;
  config::KeyPress {
    key: keystroke.key.clone(),
    ctrl: modifiers.control,
    alt: modifiers.alt,
    shift: modifiers.shift,
    platform: modifiers.platform,
  }
}

/// The press in GPUI's keystroke syntax, such as `cmd-shift-q`: the reverse of `key_press`.
pub(crate) fn gpui_keystroke(press: &config::KeyPress) -> String {
  let modifiers = [
    (press.ctrl, "ctrl-"),
    (press.alt, "alt-"),
    (press.shift, "shift-"),
    (press.platform, "cmd-"),
  ];
  let mut text = String::new();
  for (held, name) in modifiers {
    if held {
      text.push_str(name);
    }
  }
  text.push_str(&press.key);
  text
}

/// The `(row, col)` under a point in the terminal, clamped to the grid; `size` is `(cols, rows)`.
pub(crate) fn cell_at(
  x: f32,
  y: f32,
  cell_width: f32,
  line_height: f32,
  size: (u16, u16),
) -> (usize, usize) {
  let (cols, rows) = size;
  let clamp = |value: f32, cells: u16| (value.max(0.0) as usize).min(cells as usize - 1);
  (clamp(y / line_height, rows), clamp(x / cell_width, cols))
}

/// Adds up wheel deltas, which trackpads send in fractions of a line.
#[derive(Debug, Default)]
pub(crate) struct Wheel {
  remainder: f32,
}

impl Wheel {
  /// Whole lines to scroll for this delta in pixels; positive is up, into history.
  pub(crate) fn lines(&mut self, delta: f32, line_height: f32) -> i32 {
    self.remainder += delta;
    let lines = (self.remainder / line_height).trunc();
    self.remainder -= lines * line_height;
    lines as i32
  }
}

fn named_key(key: &str) -> Option<Key> {
  let named = match key {
    "enter" => Key::Enter,
    "backspace" => Key::Backspace,
    "tab" => Key::Tab,
    "escape" => Key::Escape,
    "up" => Key::Up,
    "down" => Key::Down,
    "left" => Key::Left,
    "right" => Key::Right,
    "home" => Key::Home,
    "end" => Key::End,
    "pageup" => Key::PageUp,
    "pagedown" => Key::PageDown,
    "insert" => Key::Insert,
    "delete" => Key::Delete,
    _ => return key.strip_prefix('f')?.parse().ok().map(Key::F),
  };
  Some(named)
}
