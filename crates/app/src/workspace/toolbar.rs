//! The buttons under the panes, for the active terminal, and the menu that opens from them.

use gpui::Corner;

use super::*;

/// The room under the panes: a button's height and a small gap above it. The window's own
/// padding goes below.
pub(super) const TOOLBAR_HEIGHT: f32 = BUTTON_HEIGHT + 4.0;
const BUTTON_HEIGHT: f32 = 22.0;

/// The toolbar's menu, open at `position` for the folder the terminal was in when it opened.
pub(super) struct ToolbarMenu {
  position: Point<Pixels>,
  folder: Option<String>,
  /// Fades in.
  pub(super) shown: Tween,
}

/// Where a terminal is now, asked of the core since the app only hears of changes to a terminal's
/// program and status. Blocks on the core: call it off the UI thread, or from a single key press
/// or click.
pub(super) fn terminal_dir(client: &Client, address: &str) -> Option<String> {
  let params = TerminalParams {
    terminal: address.to_string(),
  };
  let terminal: Terminal = client.call("terminal.get", &params).ok()?;
  live_dir(&terminal)
}

/// A terminal's `cwd` is where it started; its foreground program's follows every `cd`.
fn live_dir(terminal: &Terminal) -> Option<String> {
  let foreground = terminal
    .foreground
    .as_ref()
    .map(|foreground| foreground.cwd.as_str())
    .filter(|cwd| !cwd.is_empty());
  let dir = foreground.unwrap_or(&terminal.cwd);
  (!dir.is_empty()).then(|| dir.to_string())
}

impl Workspace {
  /// Where the active terminal is now. A call to the core, so only for a single key press or
  /// click, as a new split or the toolbar menu; the panel looks it up on the background executor.
  pub(super) fn working_dir(&self) -> Option<String> {
    terminal_dir(&self.client, self.layout.focused()?)
  }

  /// Opens the menu upward from the button, its right edge on the button's.
  /// The button opens the menu, and closes it when it is already open.
  fn toggle_toolbar_menu(&mut self, cx: &mut Context<Self>) {
    if self.toolbar_menu.take().is_some() {
      cx.notify();
      return;
    }
    let Some(button) = *self.more_bounds.borrow() else {
      return;
    };
    let position = point(button.right(), button.top() - px(8.0));
    self.menu = None;
    self.toolbar_menu = Some(ToolbarMenu {
      position,
      folder: self.working_dir(),
      shown: Tween::new(0.0, 1.0, Instant::now(), motion::duration()),
    });
    cx.notify();
  }

  /// Only while a terminal is active.
  pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
    self.layout.focused()?;
    let colors = Colors::active(cx);
    let bounds = Rc::clone(&self.more_bounds);
    let more = chrome_button("toolbar-more", colors)
      .size(px(BUTTON_HEIGHT))
      .relative()
      .child(glyph_icon(ICON_ELLIPSIS))
      .child(
        canvas(
          move |measured, _, _| {
            *bounds.borrow_mut() = Some(measured);
          },
          |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
      )
      .on_click(cx.listener(|workspace, _, _, cx| {
        workspace.toggle_toolbar_menu(cx);
      }));
    let panel_button = |id: &'static str,
                        panel: Panel,
                        glyph: (&'static str, f32),
                        label: &'static str,
                        cx: &mut Context<Self>| {
      chrome_button(id, colors)
        .w_auto()
        .h(px(BUTTON_HEIGHT))
        // The icon's box has room around its drawing and the text does not, so the icon side
        // takes less to look the same as the other.
        .pl(px(4.0))
        .pr(px(8.0))
        .gap_1()
        .when(self.panel == Some(panel), |button| {
          button.bg(colors.surface_active)
        })
        .child(glyph_icon(glyph))
        .child(div().text_size(px(12.0)).child(label))
        .on_click(cx.listener(move |workspace, _, _, cx| {
          workspace.toggle_panel(panel, cx);
        }))
    };
    let files = panel_button("toolbar-files", Panel::Files, ICON_FOLDER, "Files", cx);
    let git = panel_button(
      "toolbar-changes",
      Panel::Changes,
      ICON_SOURCE_CONTROL,
      "Changes",
      cx,
    );
    // Over the window's bottom right, whether or not the panel is open: it does not move.
    Some(
      div()
        .absolute()
        .bottom(px(8.0))
        .right(px(8.0))
        .h(px(BUTTON_HEIGHT))
        .flex()
        .items_center()
        // Enough that one button's highlight does not run into the next one's.
        .gap(px(4.0))
        .child(files)
        .child(git)
        .child(more),
    )
  }

  pub(super) fn render_toolbar_menu(
    &self,
    now: Instant,
    cx: &mut Context<Self>,
  ) -> Option<impl IntoElement> {
    let menu = self.toolbar_menu.as_ref()?;
    let folder = menu.folder.clone();
    let has_folder = folder.is_some();
    let editor = self.editors.first().cloned();
    let open_folder = folder.clone();
    let open_editor = folder.clone();
    let new_terminal = folder;
    let colors = Colors::active(cx);
    let menu_colors = menu_colors(colors);
    let panel = div()
      .w(px(220.0))
      .py_1()
      .rounded_lg()
      .border_1()
      .border_color(colors.surface_hover_strong)
      .bg(colors.surface_raised)
      .shadow_lg()
      .text_size(px(13.0))
      .text_color(colors.text)
      .occlude()
      // A press on the button itself is the button's: it closes the menu with its own click.
      .on_mouse_down_out(cx.listener(|workspace, event: &MouseDownEvent, _, cx| {
        let on_button = workspace
          .more_bounds
          .borrow()
          .is_some_and(|button| button.contains(&event.position));
        if !on_button {
          workspace.toolbar_menu = None;
          cx.notify();
        }
      }))
      .child(
        menu_item_icon(
          "open-folder",
          glyph(ICON_FOLDER_OPENED, colors),
          "Open Folder",
          has_folder,
          menu_colors,
        )
        .when(has_folder, |item| {
          item.on_click(cx.listener(move |workspace, _, _, cx| {
            workspace.toolbar_menu = None;
            if let Some(folder) = &open_folder {
              editors::open_folder(std::path::Path::new(folder));
            }
            cx.notify();
          }))
        }),
      )
      .children(editor.map(|editor| {
        let label = format!("Open with {}", editor.name);
        let logo = match &editor.logo {
          Some(path) => gpui::svg()
            .path(path.clone())
            .size(px(12.0))
            .text_color(rgb(editor.color))
            .into_any_element(),
          None => div()
            .text_color(rgb(editor.color))
            .child(glyph_icon(ICON_CODE))
            .into_any_element(),
        };
        menu_item_icon("open-editor", logo, &label, has_folder, menu_colors).when(
          has_folder,
          |item| {
            item.on_click(cx.listener(move |workspace, _, _, cx| {
              workspace.toolbar_menu = None;
              if let Some(folder) = &open_editor {
                editor.open(std::path::Path::new(folder));
              }
              cx.notify();
            }))
          },
        )
      }))
      .child(menu_separator(menu_colors))
      .child(
        menu_item_icon(
          "new-terminal",
          glyph(ICON_TERMINAL, colors),
          "New Terminal",
          has_folder,
          menu_colors,
        )
        .when(has_folder, |item| {
          item.on_click(cx.listener(move |workspace, _, window, cx| {
            workspace.toolbar_menu = None;
            workspace.new_tab_in(new_terminal.clone(), window, cx);
          }))
        }),
      );
    Some(deferred(
      anchored()
        .position(menu.position)
        .anchor(Corner::BottomRight)
        .snap_to_window()
        .child(div().opacity(menu.shown.value(now)).child(panel)),
    ))
  }
}
