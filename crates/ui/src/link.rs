//! The text link: a label that opens a URL, drawn in every state from the theme.

use gpui::{App, FocusHandle, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::colors::Colors;
use crate::tokens::MIN_HIT_HEIGHT;

/// Opacity of the label while it is pressed.
const PRESSED_OPACITY: f32 = 0.7;

/// A link that opens `url`. Hover and focus both underline it: a box
/// would make it look like a button.
/// Give it a focus handle to make it reachable from the keyboard: Enter and Space open it.
#[derive(IntoElement)]
pub struct Link {
  id: &'static str,
  label: SharedString,
  url: SharedString,
  focus_handle: Option<FocusHandle>,
}

impl Link {
  pub fn new(
    id: &'static str,
    label: impl Into<SharedString>,
    url: impl Into<SharedString>,
  ) -> Self {
    Self {
      id,
      label: label.into(),
      url: url.into(),
      focus_handle: None,
    }
  }

  pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
    self.focus_handle = Some(focus_handle.clone());
    self
  }
}

impl RenderOnce for Link {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let colors = Colors::active(cx);
    let focused = self
      .focus_handle
      .as_ref()
      .is_some_and(|handle| handle.is_focused(window));
    let url = self.url;
    div()
      .id(self.id)
      .when_some(self.focus_handle, |link, handle| link.track_focus(&handle))
      .flex()
      .items_center()
      .min_h(px(MIN_HIT_HEIGHT))
      .cursor_pointer()
      .text_size(px(13.0))
      .text_color(colors.text_link)
      .when(focused, |link| link.underline())
      .hover(|link| link.underline())
      .active(|link| link.opacity(PRESSED_OPACITY))
      .child(self.label)
      .on_click(move |_, _, cx| cx.open_url(&url))
  }
}
