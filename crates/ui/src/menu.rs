//! The parts every context menu shares.

use gpui::{AnyElement, Div, Hsla, SharedString, Stateful, div, prelude::*, px};

/// The colors a menu's rows and separators draw with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuColors {
  /// The line between groups of items.
  pub separator: Hsla,
  /// The background of an enabled item under the pointer.
  pub hover_background: Hsla,
  /// A disabled item's text, and an enabled item's shortcut.
  pub dim_text: Hsla,
}

/// A thin line between groups of items.
pub fn menu_separator(colors: MenuColors) -> Div {
  div().h(px(1.0)).mx_2().my_1().bg(colors.separator)
}

fn menu_row(id: &str, enabled: bool, colors: MenuColors) -> Stateful<Div> {
  div()
    .id(SharedString::from(format!("menu-{id}")))
    .mx_1()
    .px_2()
    .h(px(26.0))
    .flex()
    .items_center()
    .justify_between()
    .gap_4()
    .rounded_md()
    .when(enabled, |item| {
      item.hover(|item| item.bg(colors.hover_background))
    })
    .when(!enabled, |item| item.text_color(colors.dim_text))
}

/// An item with a label and, at the end, its shortcut. A label too long for the menu ends in an
/// ellipsis. The caller adds the click.
pub fn menu_item(
  id: &str,
  label: &str,
  shortcut: Option<String>,
  enabled: bool,
  colors: MenuColors,
) -> Stateful<Div> {
  menu_row(id, enabled, colors)
    .child(div().flex_1().min_w_0().truncate().child(label.to_string()))
    .children(shortcut.map(|shortcut| div().text_color(colors.dim_text).child(shortcut)))
}

/// An item with a 16px icon before its label.
pub fn menu_item_icon(
  id: &str,
  icon: AnyElement,
  label: &str,
  enabled: bool,
  colors: MenuColors,
) -> Stateful<Div> {
  menu_row(id, enabled, colors).child(
    div()
      .flex()
      .items_center()
      .gap_2()
      .child(
        div()
          .size(px(16.0))
          .flex_none()
          .flex()
          .items_center()
          .justify_center()
          .child(icon),
      )
      .child(label.to_string()),
  )
}
