//! The About window: the app's name, its version, the connected core's version and the repository
//! link.

use gpui::{
  App, Bounds, Context, FocusHandle, FontWeight, Global, KeyDownEvent, SharedString, Subscription,
  TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div, point, prelude::*, px,
  size,
};
use ui::colors::Colors;
use ui::link::Link;
use ui::tokens::{TITLEBAR_HEIGHT, TRAFFIC_LIGHTS_LEFT, TRAFFIC_LIGHTS_TOP};

const NAME: &str = "Visr";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY_URL: &str = "https://github.com/atleugim/visr";
const REPOSITORY_LABEL: &str = "GitHub";
/// The title bar inset (40), the heading and the versions line with their gap (about 57), a gap
/// of 8, the link row (24) and 12 of bottom padding, rounded up to the 4 px scale.
const WINDOW_HEIGHT: f32 = 144.0;

/// The open About window, kept so a second request focuses it instead of opening another.
struct AboutWindow(WindowHandle<About>);

impl Global for AboutWindow {}

/// The version of the herdr-core the app last connected to. It is absent until the first
/// connection, and the window then shows only the app's version.
struct CoreVersion(SharedString);

impl Global for CoreVersion {}

/// Records the core's version, read from its handshake, for the next About window to show.
pub(crate) fn set_core_version(version: &str, cx: &mut App) {
  cx.set_global(CoreVersion(version.to_string().into()));
}

/// Opens the About window, or brings the one that is open to the front.
pub(crate) fn open(cx: &mut App) {
  let open = cx.try_global::<AboutWindow>().map(|about| about.0);
  // A closed window fails the update, and then a new one opens.
  if let Some(handle) = open
    && handle
      .update(cx, |_, window, _| window.activate_window())
      .is_ok()
  {
    return;
  }
  let bounds = Bounds::centered(None, size(px(300.0), px(WINDOW_HEIGHT)), cx);
  let options = WindowOptions {
    window_bounds: Some(WindowBounds::Windowed(bounds)),
    titlebar: Some(TitlebarOptions {
      title: Some(format!("About {NAME}").into()),
      // Same as the main window: Windows keeps its native title bar.
      appears_transparent: cfg!(target_os = "macos"),
      traffic_light_position: Some(point(px(TRAFFIC_LIGHTS_LEFT), px(TRAFFIC_LIGHTS_TOP))),
    }),
    is_resizable: false,
    is_minimizable: false,
    ..Default::default()
  };
  let core_version = cx.try_global::<CoreVersion>().map(|core| core.0.clone());
  let opened = cx.open_window(options, |window, cx| {
    cx.new(|cx| About::new(core_version, window, cx))
  });
  // A failed open leaves the running app as it was.
  let Ok(handle) = opened else {
    return;
  };
  cx.set_global(AboutWindow(handle));
}

/// The About window's content.
struct About {
  core_version: Option<SharedString>,
  /// The link is the only thing to focus, so it holds focus from the start: Escape reaches the
  /// window through it and Enter opens the repository.
  link_focus: FocusHandle,
  /// Repaints the window when the theme changes.
  _theme: Subscription,
}

impl About {
  fn new(core_version: Option<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let link_focus = cx.focus_handle().tab_stop(true);
    window.focus(&link_focus);
    let theme = cx.observe_global::<Colors>(|_, cx| cx.notify());
    Self {
      core_version,
      link_focus,
      _theme: theme,
    }
  }

  /// Escape closes the window, and the system hands focus back to the window it came from.
  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, _: &mut Context<Self>) {
    // A held key repeats, and must not act again on the window it just opened from the menu.
    // Ctrl or Cmd with Escape is some other shortcut, which the window must not take as its own.
    let modifiers = &event.keystroke.modifiers;
    if event.is_held || modifiers.control || modifiers.platform {
      return;
    }
    if event.keystroke.key == "escape" {
      window.remove_window();
    }
  }
}

impl Render for About {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let colors = Colors::active(cx);
    div()
      .id("about")
      .on_key_down(cx.listener(Self::on_key_down))
      .size_full()
      .flex()
      .flex_col()
      .items_center()
      .bg(colors.surface)
      .text_color(colors.text)
      .child(
        div()
          .pt(px(TITLEBAR_HEIGHT))
          .flex()
          .flex_col()
          .items_center()
          .gap_1()
          .child(
            div()
              .text_size(px(20.0))
              .font_weight(FontWeight::BOLD)
              .child(NAME),
          )
          .child(
            // Separate children, so the gap on each side of the dot is a scale step, not spaces.
            div()
              .flex()
              .items_center()
              .gap_2()
              .text_size(px(13.0))
              .text_color(colors.text_muted)
              .child(format!("Version {VERSION}"))
              .when_some(self.core_version.as_ref(), |row, core| {
                row.child("·").child(format!("herdr-core {core}"))
              }),
          ),
      )
      .child(
        div().pt_2().pb_3().child(
          Link::new("about-repository", REPOSITORY_LABEL, REPOSITORY_URL)
            .track_focus(&self.link_focus),
        ),
      )
  }
}
