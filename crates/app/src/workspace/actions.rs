//! What the person can do: shortcuts, tabs, splits, the sidebar toggle and divider drags.

use super::*;

impl Workspace {
  /// A new tab, its shell starting in `cwd` (the home folder when `None`).
  pub(super) fn new_tab_in(
    &mut self,
    cwd: Option<String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.open_tab(Vec::new(), cwd, cx) {
      self.changed(window, cx);
    }
  }

  /// A new tab running `argv` (the shell when empty) in `cwd` (the home folder when `None`),
  /// sized from the layout. Says whether it started.
  pub(super) fn open_tab(&mut self, argv: Vec<String>, cwd: Option<String>, cx: &App) -> bool {
    let size = self
      .new_pane_cells(None, cx)
      .or_else(|| self.focused_size(cx));
    let Some(address) = self.start(argv, size, cwd) else {
      return false;
    };
    self.layout.new_tab_in_active_folder(&address);
    true
  }

  /// The focused pane's size in cells, for when the layout's area is not measured yet.
  fn focused_size(&self, cx: &App) -> Option<(u16, u16)> {
    self.focused_view().map(|view| view.read(cx).size())
  }

  /// The cells a new pane will hold once the layout settles: a split of the focused pane, or a
  /// whole tab when `split` is `None`. Worked out from the layout rather than read off the
  /// focused grid, which is stale while panes move or one is zoomed. A little under, never over:
  /// a shell that starts wider than its pane prints a stray `%` and a blank line.
  fn new_pane_cells(&self, split: Option<Axis>, cx: &App) -> Option<(u16, u16)> {
    let area = (*self.panes_area.borrow())?;
    // The area as it will be once the sidebar and the panel stop moving.
    let now = Instant::now();
    let still_to_go = |drawn: f32, shown: f32, open: bool| drawn * (shown - f32::from(open));
    let width = f32::from(area.size.width)
      + still_to_go(
        self.sidebar_drawn,
        self.sidebar.value(now),
        self.sidebar_open,
      )
      + still_to_go(
        self.panel_drawn,
        self.panel_motion.value(now),
        self.panel.is_some(),
      );
    let area = (width, f32::from(area.size.height));
    let (width, height) = match split {
      None => area,
      Some(axis) => {
        let focused = self.layout.focused()?;
        let (width, height) = self
          .layout
          .active()?
          .root
          .extent_of(focused, area, DIVIDER_WIDTH)?;
        match axis {
          Axis::Horizontal => ((width - DIVIDER_WIDTH) / 2.0, height),
          Axis::Vertical => (width, (height - DIVIDER_WIDTH) / 2.0),
        }
      }
    };
    let (cell_width, line_height) = self.focused_view()?.read(cx).cell_size();
    let cols = ((width - PANE_CHROME_WIDTH) / cell_width).floor() - 1.0;
    let rows = ((height - PANE_CHROME_HEIGHT) / line_height).floor();
    Some((cols.max(1.0) as u16, rows.max(1.0) as u16))
  }

  fn focused_view(&self) -> Option<Entity<TerminalView>> {
    let focused = self.layout.focused()?;
    self.panes.get(focused).map(|pane| pane.view.clone())
  }

  pub(super) fn run_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
    match action {
      Action::Copy => {
        if let Some(view) = self.focused_view() {
          view.update(cx, |view, cx| view.copy(cx));
        }
        return;
      }
      Action::Paste => {
        if let Some(view) = self.focused_view() {
          view.update(cx, |view, cx| view.paste(cx));
        }
        return;
      }
      Action::ReloadTheme => {
        self.reload_theme(window, cx);
        return;
      }
      Action::Quit => {
        cx.quit();
        return;
      }
      Action::NewTab => {
        self.open_tab(Vec::new(), None, cx);
      }
      Action::SplitRight | Action::SplitDown => {
        // A new pane belongs beside the others, so the tab shows them all again.
        self.zoom = None;
        let axis = if action == Action::SplitRight {
          Axis::Horizontal
        } else {
          Axis::Vertical
        };
        let size = self
          .new_pane_cells(Some(axis), cx)
          .or_else(|| self.focused_size(cx).map(|size| split_size(size, axis)));
        if let Some(address) = self.start(Vec::new(), size, self.working_dir()) {
          self.layout.split(axis, &address);
          // The new pane grows out of the one it splits.
          let path = self
            .layout
            .active()
            .and_then(|tab| tab.root.path_to(&address));
          if let Some(mut path) = path
            && path.pop().is_some()
          {
            let tween = Tween::new(1.0, 0.5, Instant::now(), motion::duration());
            self.split_motion = Some((self.layout.active_tab, path, tween));
          }
        }
      }
      // Closing ends the terminal too: a pane closed away would only come back in the sidebar.
      Action::ClosePane => {
        if let Some(focused) = self.layout.focused().map(str::to_string) {
          self.close_address(&focused);
          self.close_pane(&focused, window, cx);
        }
      }
      Action::CloseTab => {
        let shown: Vec<String> = self
          .layout
          .active()
          .map(|tab| tab.root.leaves().into_iter().map(str::to_string).collect())
          .unwrap_or_default();
        for address in &shown {
          self.close_address(address);
        }
        self.layout.close_tab();
      }
      Action::FocusNext => self.layout.focus_next(),
      Action::FocusPrev => self.layout.focus_prev(),
      Action::SelectTab(index) => self.layout.select_tab(index),
      Action::ToggleSidebar => self.toggle_sidebar(),
      Action::ZoomPane => {
        if let Some(focused) = self.layout.focused().map(str::to_string) {
          self.toggle_zoom(&focused, window, cx);
        }
      }
      Action::MoveTabUp | Action::MoveTabDown => {
        if let Some(row) = self.active_row() {
          let direction = if action == Action::MoveTabUp {
            Direction::Up
          } else {
            Direction::Down
          };
          self.move_row(&row, direction);
        }
      }
      Action::MoveToNewFolder => {
        if let Some(row) = self.active_row() {
          // The new folder's name is edited in the sidebar, so it has to be in sight.
          if !self.sidebar_open {
            self.toggle_sidebar();
          }
          self.move_to_new_folder(&row, window, cx);
        }
      }
      Action::NewFolder => {
        if !self.sidebar_open {
          self.toggle_sidebar();
        }
        self.new_folder(window, cx);
      }
      Action::ToggleFolder => {
        if let Some(id) = self.active_folder() {
          self.layout.toggle_folder(id);
        }
      }
      Action::RemoveFromFolder => {
        if let Some(row) = self.active_row() {
          self.remove_from_folder(&row);
        }
      }
    }
    self.changed(window, cx);
  }

  /// Comes in from a curtain of the window's color, for the setup to hand over to it.
  pub(crate) fn fade_in(&mut self) {
    self.entrance = Tween::new(0.0, 1.0, Instant::now(), motion::duration());
  }

  pub(super) fn toggle_sidebar(&mut self) {
    self.sidebar_open = !self.sidebar_open;
    let target = if self.sidebar_open { 1.0 } else { 0.0 };
    self.sidebar = self
      .sidebar
      .retarget(target, Instant::now(), motion::duration());
  }

  pub(super) fn shortcut(&self, action: Action) -> Option<String> {
    self.config.keys.binding(action).map(KeyPress::label)
  }

  /// Runs before the focused terminal sees the key, so shortcuts never reach the program.
  pub(super) fn on_key_down(
    &mut self,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    // A row or folder being renamed is a text field: cmd-c, cmd-v and the rest are its own.
    if self.renaming.is_some() {
      return;
    }
    if let Some(action) = self.config.keys.action(&input::key_press(&event.keystroke)) {
      cx.stop_propagation();
      self.run_action(action, window, cx);
    }
  }

  pub(super) fn on_drag_move(
    &mut self,
    event: &MouseMoveEvent,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.drag_edge(event, f32::from(window.viewport_size().width), cx) {
      return;
    }
    let Some((path, axis)) = &self.dragging else {
      return;
    };
    if event.pressed_button != Some(MouseButton::Left) {
      self.dragging = None;
      return;
    }
    let Some(bounds) = self.split_bounds.borrow().get(path).copied() else {
      return;
    };
    let ratio = match axis {
      Axis::Horizontal => (event.position.x - bounds.origin.x) / bounds.size.width,
      Axis::Vertical => (event.position.y - bounds.origin.y) / bounds.size.height,
    };
    let path = path.clone();
    self.layout.set_ratio(&path, ratio);
    cx.notify();
  }

  pub(super) fn on_drag_end(
    &mut self,
    _event: &MouseUpEvent,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.edge_drag.take().is_some() {
      cx.notify();
    }
    if self.dragging.take().is_some() {
      self.save();
      cx.notify();
    }
  }
}
