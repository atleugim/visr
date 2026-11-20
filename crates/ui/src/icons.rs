//! Icons drawn from boxes, strokes and glyphs, so they center like each other.

use gpui::{
  AnyElement, Axis, Div, Hsla, IntoElement, MouseButton, Stateful, canvas, div, point, prelude::*,
  px,
};

use crate::tokens::{ICON_BOX, TITLEBAR_BUTTON};

/// The bundled Nerd Font the glyph icons come from.
pub const ICON_FONT: &str = "Lilex Nerd Font Mono";
/// Material Design icons: diagonal arrows out to expand a pane, in to put it back.
pub const ICON_EXPAND: &str = "\u{f0616}";
pub const ICON_RESTORE: &str = "\u{f0615}";
/// Each with the size that makes it look as big as the others: the glyphs fill their boxes
/// differently.
pub const ICON_FOLDER: (&str, f32) = ("\u{ea83}", 19.0);
pub const ICON_SOURCE_CONTROL: (&str, f32) = ("\u{ea68}", 15.0);
pub const ICON_ELLIPSIS: (&str, f32) = ("\u{ea7c}", 19.0);
pub const ICON_FOLDER_OPENED: (&str, f32) = ("\u{eaf7}", 20.0);
pub const ICON_TERMINAL: (&str, f32) = ("\u{ea85}", 20.0);
/// For an editor without a logo of its own.
pub const ICON_CODE: (&str, f32) = ("\u{eac4}", 20.0);

/// A codicon centered in a box of one size, so every toolbar icon sits in the same place.
pub fn glyph_icon((glyph, size): (&'static str, f32)) -> impl IntoElement {
  div()
    .size(px(ICON_BOX))
    .overflow_hidden()
    .flex()
    .items_center()
    .justify_center()
    .font_family(ICON_FONT)
    .text_size(px(size))
    .child(glyph)
}

/// A small window outline cut in two along `axis`, drawn with boxes so it needs no icon font.
pub fn split_icon(axis: Axis, color: Hsla) -> AnyElement {
  let line = div().bg(color);
  let line = match axis {
    Axis::Horizontal => line.w(px(1.0)).h_full(),
    Axis::Vertical => line.h(px(1.0)).w_full(),
  };
  div()
    .w(px(13.0))
    .h(px(11.0))
    .border_1()
    .border_color(color)
    .rounded_sm()
    .flex()
    .when(axis == Axis::Vertical, |icon| icon.flex_col())
    .justify_center()
    .items_center()
    .child(line)
    .into_any_element()
}

/// A window with its left panel marked off: the sidebar.
pub fn sidebar_icon(color: Hsla) -> impl IntoElement {
  div()
    .w(px(15.0))
    .h(px(12.0))
    .flex_none()
    .border_1()
    .border_color(color)
    .rounded_sm()
    .flex()
    .child(div().w(px(4.0)).h_full().border_r_1().border_color(color))
}

/// A square button for the title bar and the toolbar, with its icon as the child. It fills with
/// `hover_background` under the pointer. The caller sets the text color.
pub fn titlebar_button(id: &'static str, hover_background: Hsla) -> Stateful<Div> {
  div()
    .id(id)
    .size(px(TITLEBAR_BUTTON))
    .flex()
    .items_center()
    .justify_center()
    .rounded_md()
    .cursor_pointer()
    .hover(move |button| button.bg(hover_background))
    // A quick second click is the button's, not a double click on the title bar.
    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// An X drawn as two strokes: a typed × sits below the middle in most fonts.
pub fn close_icon(color: Hsla) -> impl IntoElement {
  let size = 9.0;
  canvas(
    |_, _, _| {},
    move |bounds, _, window, _| {
      let mut path = gpui::PathBuilder::stroke(px(1.25));
      let (left, top) = (bounds.origin.x, bounds.origin.y);
      let (right, bottom) = (left + px(size), top + px(size));
      path.move_to(point(left, top));
      path.line_to(point(right, bottom));
      path.move_to(point(right, top));
      path.line_to(point(left, bottom));
      if let Ok(path) = path.build() {
        window.paint_path(path, color);
      }
    },
  )
  .size(px(size))
  .flex_none()
}

/// Drawn rather than typed, so it centers exactly like the other icons.
pub fn plus_icon(color: Hsla) -> impl IntoElement {
  let (size, stroke) = (12.0, 1.5);
  let middle = (size - stroke) / 2.0;
  div()
    .relative()
    .size(px(size))
    .child(
      div()
        .absolute()
        .left_0()
        .top(px(middle))
        .w_full()
        .h(px(stroke))
        .bg(color),
    )
    .child(
      div()
        .absolute()
        .top_0()
        .left(px(middle))
        .h_full()
        .w(px(stroke))
        .bg(color),
    )
}

/// The colors of a shell's icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellColors {
  pub background: Hsla,
  pub text: Hsla,
}

/// The `>_` of a terminal on a small tile.
pub fn shell_icon(colors: ShellColors) -> impl IntoElement {
  div()
    .size(px(ICON_BOX))
    .flex_none()
    .flex()
    .items_center()
    .justify_center()
    .rounded_sm()
    .bg(colors.background)
    .text_size(px(10.0))
    .text_color(colors.text)
    .child(">_")
}

/// The colors of a stack icon: the shell in front, the accent of the one behind it, and the
/// outline that separates them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackColors {
  pub shell: ShellColors,
  pub behind: Hsla,
  pub outline: Hsla,
}

/// Two shells, one behind the other: a tab split into several panes.
pub fn stack_icon(colors: StackColors) -> AnyElement {
  let layer = || {
    div()
      .absolute()
      .size(px(15.0))
      .rounded_sm()
      .border_1()
      .border_color(colors.outline)
  };
  div()
    .relative()
    .size(px(ICON_BOX))
    .flex_none()
    .child(layer().top_0().right_0().bg(colors.behind))
    .child(
      layer()
        .bottom_0()
        .left_0()
        .bg(colors.shell.background)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(9.0))
        .text_color(colors.shell.text)
        .child(">_"),
    )
    .into_any_element()
}
