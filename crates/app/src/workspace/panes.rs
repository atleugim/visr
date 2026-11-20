//! The active tab's panes: the split tree, the pane cards, and what moves them (zoom, closing).

use super::*;

pub(super) fn path_id(prefix: &str, path: &[Branch]) -> SharedString {
  let steps: String = path
    .iter()
    .map(|branch| match branch {
      Branch::First => '0',
      Branch::Second => '1',
    })
    .collect();
  format!("{prefix}-{steps}").into()
}

/// An invisible layer that reports where the element it sits in was laid out.
fn measure(on_bounds: impl FnOnce(Bounds<Pixels>) + 'static) -> impl IntoElement {
  canvas(move |bounds, _, _| on_bounds(bounds), |_, _, _, _| {})
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A header's left padding. The top left one makes room, as the sidebar goes, for what is fixed
/// at the window's corner (the traffic lights on macOS, and the sidebar toggle): its title starts
/// a gap past the toggle.
fn header_left(top_left: bool, hidden: f32) -> f32 {
  let room = if top_left {
    TOGGLE_LEFT + TITLEBAR_BUTTON + TOGGLE_GAP - PANE_EDGE - HEADER_PADDING
  } else {
    0.0
  };
  HEADER_PADDING + room * hidden
}

/// What the empty state says. The + is in the sidebar, so it is only named while that is open.
fn empty_message(sidebar_open: bool, keys: Option<&str>) -> String {
  let start = match (sidebar_open, keys) {
    (true, Some(keys)) => format!("Press + or {keys}"),
    (true, None) => "Press +".to_string(),
    (false, Some(keys)) => format!("Press {keys}"),
    (false, None) => "Open the sidebar and press +".to_string(),
  };
  format!("No terminal open. {start} to start a shell.")
}

impl Workspace {
  /// What shows when no terminal is open. Its header is part of the title bar.
  pub(super) fn render_empty(&self, cx: &App) -> AnyElement {
    let colors = Colors::active(cx);
    let header = div()
      .h(px(PANE_HEADER_HEIGHT))
      .flex_none()
      .on_mouse_down(MouseButton::Left, on_titlebar_mouse_down);
    div()
      .size_full()
      .flex()
      .flex_col()
      .rounded(px(PANE_RADIUS))
      .border(px(PANE_BORDER_WIDTH))
      .border_color(colors.border)
      .text_color(colors.text_muted)
      .child(header)
      .child(
        div()
          .flex_1()
          .flex()
          .items_center()
          .justify_center()
          .child(empty_message(
            self.sidebar_open,
            self.shortcut(Action::NewTab).as_deref(),
          )),
      )
      .into_any_element()
  }

  /// `edges` says which window edges the node touches, top and left.
  pub(super) fn render_node(
    &self,
    node: &Node,
    path: Vec<Branch>,
    edges: (bool, bool),
    cx: &mut Context<Self>,
  ) -> AnyElement {
    match node {
      Node::Leaf { terminal } => match self.panes.get(terminal) {
        // Its view is drawn over the tab; its place keeps an empty card to grow from.
        Some(_) if self.zoomed().is_some_and(|zoom| &zoom.address == terminal) => {
          self.pane_placeholder(terminal, cx)
        }
        Some(pane) => self.render_pane(terminal, pane, edges, false, cx),
        None => div().size_full().into_any_element(),
      },
      Node::Split {
        axis,
        ratio,
        first,
        second,
      } => {
        let axis = *axis;
        let horizontal = axis == Axis::Horizontal;
        let ratio = self.moving_ratio(&path).unwrap_or(*ratio);
        let first_path = [path.as_slice(), &[Branch::First]].concat();
        let (top, left) = edges;
        let second_edges = if horizontal {
          (top, false)
        } else {
          (false, left)
        };
        let second_path = [path.as_slice(), &[Branch::Second]].concat();
        let store = Rc::clone(&self.split_bounds);
        let measured = path.clone();
        let measure = measure(move |bounds| {
          store.borrow_mut().insert(measured, bounds);
        });
        let dragged = path.clone();
        let divider = div()
          .id(path_id("divider", &path))
          .flex_none()
          .when(horizontal, |divider| {
            divider.w(px(DIVIDER_WIDTH)).h_full().cursor_col_resize()
          })
          .when(!horizontal, |divider| {
            divider.h(px(DIVIDER_WIDTH)).w_full().cursor_row_resize()
          })
          .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |workspace, _, _, cx| {
              cx.stop_propagation();
              workspace.dragging = Some((dragged.clone(), axis));
            }),
          );
        let first = div()
          .overflow_hidden()
          .min_w_0()
          .min_h_0()
          .when(horizontal, |pane| pane.w(relative(ratio)).h_full())
          .when(!horizontal, |pane| pane.h(relative(ratio)).w_full())
          .child(self.render_node(first, first_path, edges, cx));
        let second = div()
          .flex_1()
          .overflow_hidden()
          .min_w_0()
          .min_h_0()
          .child(self.render_node(second, second_path, second_edges, cx));
        div()
          .relative()
          .size_full()
          .flex()
          .when(horizontal, |split| split.flex_row())
          .when(!horizontal, |split| split.flex_col())
          .child(measure)
          .child(first)
          .child(divider)
          .child(second)
          .into_any_element()
      }
    }
  }

  pub(super) fn pane_button(
    &self,
    address: &str,
    action: Action,
    icon: AnyElement,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let colors = Colors::active(cx);
    let target = address.to_string();
    div()
      .id(SharedString::from(format!("{action:?}-{address}")))
      .size(px(22.0))
      .flex()
      .items_center()
      .justify_center()
      .rounded_md()
      .hover(|button| button.bg(colors.surface_hover))
      .child(icon)
      .on_click(cx.listener(move |workspace, _, window, cx| {
        cx.stop_propagation();
        workspace.layout.focus(&target);
        workspace.run_action(action, window, cx);
      }))
      .into_any_element()
  }

  /// Measures where a pane sits, for a zoom to grow from it.
  pub(super) fn measure_pane(&self, address: &str) -> impl IntoElement {
    let store = Rc::clone(&self.pane_bounds);
    let address = address.to_string();
    measure(move |bounds| {
      store.borrow_mut().insert(address, bounds);
    })
  }

  pub(super) fn pane_placeholder(&self, address: &str, cx: &App) -> AnyElement {
    let colors = Colors::active(cx);
    div()
      .relative()
      .size_full()
      .rounded(px(PANE_RADIUS))
      .border(px(PANE_BORDER_WIDTH))
      .border_color(colors.border)
      .child(self.measure_pane(address))
      .into_any_element()
  }

  /// `zoomed` is the copy drawn over the tab: it offers restore instead of expand.
  pub(super) fn render_pane(
    &self,
    address: &str,
    pane: &Pane,
    (top, left): (bool, bool),
    zoomed: bool,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    // With the sidebar hidden, the top left pane's header takes the title bar's place: it leaves
    // room for the traffic lights and the fixed sidebar toggle. The room grows as the sidebar goes.
    let hidden = 1.0 - self.sidebar.value(Instant::now());
    let header_left = header_left(top && left, hidden);
    let is_focused = self.layout.focused() == Some(address);
    let closing = self
      .closing_pane
      .as_ref()
      .filter(|closing| closing.address == address);
    let title = match (closing, self.terminals.get(address)) {
      (Some(closing), _) => closing.title.clone(),
      (None, Some(terminal)) => display_name(terminal),
      (None, None) => address.to_string(),
    };
    let colors = Colors::active(cx);
    let palette = TerminalColors::palette(cx);
    let dim = colors.text_muted;
    let split = self
      .layout
      .active()
      .is_some_and(|tab| tab.root.leaves().len() > 1);
    let zoom_icon = if zoomed { ICON_RESTORE } else { ICON_EXPAND };
    let zoom_button = split.then(|| {
      self.pane_button(
        address,
        Action::ZoomPane,
        div()
          .font_family(ICON_FONT)
          .text_size(px(14.0))
          .text_color(dim)
          .child(zoom_icon)
          .into_any_element(),
        cx,
      )
    });
    // The focused pane shows its buttons; the others show them on hover.
    let buttons = div()
      .flex()
      .gap_1()
      .when(!is_focused, |buttons| {
        buttons
          .opacity(0.0)
          .group_hover("pane-header", |buttons| buttons.opacity(1.0))
      })
      .child(self.pane_button(
        address,
        Action::SplitRight,
        split_icon(gpui::Axis::Horizontal, dim),
        cx,
      ))
      .child(self.pane_button(
        address,
        Action::SplitDown,
        split_icon(gpui::Axis::Vertical, dim),
        cx,
      ))
      .children(zoom_button)
      .child(self.pane_button(
        address,
        Action::ClosePane,
        close_icon(dim).into_any_element(),
        cx,
      ));
    let focused = address.to_string();
    let header = div()
      .group("pane-header")
      .h(px(PANE_HEADER_HEIGHT))
      .flex_none()
      .flex()
      .items_center()
      .gap_2()
      .pl(px(header_left))
      .pr_1()
      .text_size(px(13.0))
      .text_color(if is_focused {
        hsla(palette.foreground)
      } else {
        dim
      })
      .child(div().text_color(dim).child(">_"))
      .child(div().flex_1().min_w_0().truncate().child(title))
      .child(buttons)
      .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |workspace, event: &MouseDownEvent, window, cx| {
          // A header along the top is part of the title bar too.
          if top {
            on_titlebar_mouse_down(event, window, cx);
          }
          workspace.layout.focus(&focused);
          workspace.changed(window, cx);
        }),
      );
    div()
      .size_full()
      .flex()
      .flex_col()
      .overflow_hidden()
      .rounded(px(PANE_RADIUS))
      .border(px(PANE_BORDER_WIDTH))
      .border_color(if is_focused {
        colors.border_focused
      } else {
        colors.border
      })
      .bg(hsla(palette.background))
      .when(!zoomed, |card| {
        card.relative().child(self.measure_pane(address))
      })
      .child(header)
      .child(
        div()
          .flex_1()
          .min_h_0()
          .pl(px(PANE_PADDING_LEFT))
          .pb(px(PANE_PADDING_BOTTOM))
          .child(pane.view.clone()),
      )
      .into_any_element()
  }

  pub(super) fn measure_area(&self) -> impl IntoElement {
    let store = Rc::clone(&self.panes_area);
    measure(move |bounds| {
      *store.borrow_mut() = Some(bounds);
    })
  }

  /// The zoomed pane, over the tab: from its place to the whole area as the zoom eases in.
  pub(super) fn render_zoom(
    &self,
    now: Instant,
    cx: &mut Context<Self>,
  ) -> Option<impl IntoElement> {
    let zoom = self.zoomed()?;
    let pane = self.panes.get(&zoom.address)?;
    let area = (*self.panes_area.borrow())?;
    let from = self.pane_bounds.borrow().get(&zoom.address).copied()?;
    let t = zoom.tween.value(now);
    let ease = |start: Pixels, end: Pixels| start + (end - start) * t;
    Some(
      div()
        .absolute()
        .left(ease(from.origin.x - area.origin.x, px(0.0)))
        .top(ease(from.origin.y - area.origin.y, px(0.0)))
        .w(ease(from.size.width, area.size.width))
        .h(ease(from.size.height, area.size.height))
        .rounded(px(PANE_RADIUS))
        .shadow_lg()
        .child(self.render_pane(&zoom.address, pane, (true, true), true, cx)),
    )
  }

  /// The ratio a split is drawn at while it eases in.
  pub(super) fn moving_ratio(&self, path: &[Branch]) -> Option<f32> {
    let now = Instant::now();
    let active = self.layout.active_tab;
    if let Some(closing) = &self.closing_pane
      && closing.tab == active
      && closing.path == path
    {
      return Some(closing.tween.value(now));
    }
    let (tab, moving, tween) = self.split_motion.as_ref()?;
    (*tab == active && moving == path).then(|| tween.value(now))
  }

  /// The pane zoomed in the active tab, while it still is.
  pub(super) fn zoomed(&self) -> Option<&Zoom> {
    let zoom = self.zoom.as_ref()?;
    let shown = self.layout.active()?.root.leaves();
    (zoom.tab == self.layout.active_tab
      && shown.len() > 1
      && shown.contains(&zoom.address.as_str()))
    .then_some(zoom)
  }

  /// Draws the pane over its tab, or puts it back where it was.
  pub(super) fn toggle_zoom(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
    let now = Instant::now();
    let zoomed_here = self.zoomed().is_some_and(|zoom| zoom.address == address);
    if zoomed_here && let Some(zoom) = self.zoom.as_mut() {
      zoom.tween = zoom.tween.retarget(0.0, now, motion::duration());
      zoom._done = Some(cx.spawn_in(window, async move |this, cx| {
        cx.background_executor().timer(motion::duration()).await;
        let _ = this.update(cx, |workspace, cx| {
          workspace.zoom = None;
          cx.notify();
        });
      }));
    } else {
      self.zoom = Some(Zoom {
        tab: self.layout.active_tab,
        address: address.to_string(),
        tween: Tween::new(0.0, 1.0, now, motion::duration()),
        _done: None,
      });
      self.layout.focus(address);
    }
    cx.notify();
  }

  pub(super) fn is_closing(&self, address: &str) -> bool {
    self
      .closing_pane
      .as_ref()
      .is_some_and(|closing| closing.address == address)
  }

  /// Takes the pane out of the layout. In the active tab its split folds away first; a pane
  /// alone in its tab, or one closing while another is, goes at once.
  pub(super) fn close_pane(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
    let side = self.layout.active().and_then(|tab| {
      let mut path = tab.root.path_to(address)?;
      let branch = path.pop()?;
      let ratio = tab.root.ratio_at(&path)?;
      Some((path, branch, ratio))
    });
    let Some((path, branch, ratio)) = side.filter(|_| self.closing_pane.is_none()) else {
      self.layout.stop_showing(address);
      return;
    };
    let gone = if branch == Branch::First { 0.0 } else { 1.0 };
    let title = self
      .terminals
      .get(address)
      .map(display_name)
      .unwrap_or_else(|| address.to_string());
    // The pane that stays takes the focus now, not when the other is gone.
    if self.layout.focused() == Some(address) {
      self.layout.focus_prev();
    }
    let done = cx.spawn_in(window, async move |this, cx| {
      cx.background_executor().timer(motion::duration()).await;
      let _ = this.update_in(cx, |workspace, window, cx| {
        if let Some(closing) = workspace.closing_pane.take() {
          workspace.layout.stop_showing(&closing.address);
          workspace.changed(window, cx);
        }
      });
    });
    self.closing_pane = Some(ClosingPane {
      tab: self.layout.active_tab,
      path,
      address: address.to_string(),
      title,
      tween: Tween::new(ratio, gone, Instant::now(), motion::duration()),
      _done: done,
    });
    cx.notify();
  }
}
