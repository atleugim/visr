//! Drawing the workspace: the frame's motion bookkeeping and the whole window's element tree.

use super::*;

impl Workspace {
  /// Grows in rows new to the sidebar, and drops the motions that are done.
  fn track_motion(&mut self, now: Instant) {
    let rows: HashSet<String> = sidebar_entries(&self.layout, &self.terminals)
      .iter()
      .filter_map(|entry| row_key(entry).map(str::to_string))
      .collect();
    if let Some(known) = &self.known_rows {
      for row in rows.difference(known) {
        let tween = Tween::new(0.0, 1.0, now, motion::duration());
        self.row_motion.insert(row.clone(), tween);
      }
    }
    self.row_motion.retain(|_, tween| !tween.done(now));
    self.known_rows = Some(rows);
  }
}

impl Render for Workspace {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let now = Instant::now();
    let colors = Colors::active(cx);
    if self
      .split_motion
      .as_ref()
      .is_some_and(|(_, _, tween)| tween.done(now))
    {
      self.split_motion = None;
    }
    self.track_motion(now);
    let zooming = self.zoom.as_ref().is_some_and(|zoom| !zoom.tween.done(now));
    let resizing = !self.sidebar.done(now)
      || self.edge_drag.is_some()
      || !self.panel_motion.done(now)
      || self.split_motion.is_some()
      || self.closing_pane.is_some()
      || zooming;
    if self
      .panel_switch
      .as_ref()
      .is_some_and(|switch| switch.tween.done(now))
    {
      self.panel_switch = None;
    }
    let fading = !self.entrance.done(now)
      || self.panel_switch.is_some()
      || !self.row_motion.is_empty()
      || self.menu.as_ref().is_some_and(|menu| !menu.shown.done(now))
      || self
        .toolbar_menu
        .as_ref()
        .is_some_and(|menu| !menu.shown.done(now));
    if resizing || fading {
      window.request_animation_frame();
    }
    // Resizing a grid every frame reflows its text and makes programs redraw mid-way; panes keep
    // their size while things move and take the final one when they stop.
    for pane in self.panes.values() {
      pane.view.update(cx, |view, _| view.set_frozen(resizing));
    }
    let shown = self.sidebar.value(now);
    let panel_shown = self.panel_motion.value(now);
    (self.sidebar_drawn, self.panel_drawn) = resize::fit_widths(
      f32::from(window.viewport_size().width),
      (self.sidebar_width, self.sidebar_open),
      (self.panel_width, self.panel.is_some()),
    );
    let panes = match self.layout.active() {
      Some(tab) => self.render_node(&tab.root, Vec::new(), (true, true), cx),
      None => self.render_empty(cx),
    };
    // With no pane to hold the keyboard, the window does, or no shortcut would get to run.
    if self.layout.active().is_none() && !self.focus_handle.contains_focused(window, cx) {
      window.focus(&self.focus_handle);
    }
    let config_error = self.config_error.as_ref().map(|error| {
      banner(
        format!("Config not applied, keeping the last good one: {error}"),
        colors,
      )
    });
    let theme_notice = self
      .theme_notice
      .clone()
      .map(|notice| banner(notice, colors));
    let reconnecting = self
      .reconnecting
      .then(|| banner("Lost the connection to herdr-core. Reconnecting…", colors));
    let main = div()
      .flex_1()
      .min_w_0()
      .h_full()
      .flex()
      .flex_col()
      .pb(px(WINDOW_GAP))
      .pr(px(WINDOW_GAP))
      .pt(px(WINDOW_GAP))
      // Hidden, the sidebar leaves the same margin the other edges have.
      .pl(px(WINDOW_GAP * (1.0 - shown)))
      .child(
        div()
          .relative()
          .flex_1()
          .min_h_0()
          .child(panes)
          .child(self.measure_area())
          .children(self.render_zoom(now, cx)),
      )
      .children(reconnecting)
      .children(config_error)
      .children(theme_notice)
      // The toolbar is drawn over the window's corner; this keeps its room.
      .when(self.layout.focused().is_some(), |main| {
        main.child(div().h(px(TOOLBAR_HEIGHT)).flex_none())
      });
    div()
      .track_focus(&self.focus_handle)
      .capture_key_down(cx.listener(Self::on_key_down))
      .on_mouse_move(cx.listener(Self::on_drag_move))
      .on_mouse_up(MouseButton::Left, cx.listener(Self::on_drag_end))
      .relative()
      .flex()
      .size_full()
      .bg(colors.surface)
      .text_color(colors.text)
      .when(shown > 0.0, |root| {
        root.child(
          div()
            .w(px(self.sidebar_drawn * shown))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .child(self.render_sidebar(cx)),
        )
      })
      .child(main)
      .when(panel_shown > 0.0, |root| {
        root.child(
          div()
            .w(px(self.panel_drawn * panel_shown))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .child(
              div()
                .w(px(self.panel_drawn))
                .h_full()
                .child(self.render_panel(now, cx)),
            ),
        )
      })
      .when(shown > 0.0, |root| {
        root.child(
          self
            .edge_handle(Edge::Sidebar, cx)
            .left(px(self.sidebar_drawn * shown - 6.0)),
        )
      })
      .when(panel_shown > 0.0, |root| {
        root.child(
          self
            .edge_handle(Edge::Panel, cx)
            .right(px(self.panel_drawn * panel_shown + 2.0)),
        )
      })
      .children(self.render_toolbar(cx))
      .children(self.render_toolbar_menu(now, cx))
      // The one sidebar toggle: fixed in the corner and over everything but the menus, which are
      // deferred, so it never moves with the sidebar nor shows twice.
      .child(
        div()
          .absolute()
          .top(px(TOGGLE_TOP))
          .left(px(TOGGLE_LEFT))
          .child(self.sidebar_toggle(cx)),
      )
      .when(!self.entrance.done(now), |root| {
        root.child(
          div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(colors.surface)
            .opacity(1.0 - self.entrance.value(now)),
        )
      })
      .children(self.menu.as_ref().map(|menu| {
        deferred(
          anchored().position(menu.position).snap_to_window().child(
            div()
              .opacity(menu.shown.value(now))
              .child(self.render_menu(menu, cx)),
          ),
        )
      }))
  }
}

fn banner(message: impl Into<SharedString>, colors: Colors) -> impl IntoElement {
  div()
    .flex_none()
    .mt_2()
    .px_3()
    .py_1()
    .rounded_md()
    .bg(colors.surface_error)
    .text_size(px(12.0))
    .child(message.into())
}
