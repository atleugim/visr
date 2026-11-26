//! Dragging a sidebar line: the faded copy that follows the pointer, where the drag aims, and the
//! drop. A session lands between two lines, into a folder or at the end; a folder lands among the
//! top-level lines.

use super::*;

/// How far left of the pointer the dragged copy starts, so the pointer sits on it.
const GRAB_LEAD: f32 = 12.0;

/// A sidebar line on its way to a new place: what the drag carries.
#[derive(Clone)]
pub(super) struct DraggedLine {
  pub(super) key: SidebarKey,
  pub(super) name: SharedString,
}

/// The faded copy of a dragged line that follows the pointer. GPUI draws it from where the line's
/// left edge was, so it keeps how far right of that edge the line was grabbed, to start under the
/// pointer instead.
pub(super) struct DragPreview {
  name: SharedString,
  grab: Pixels,
}

/// Which edge of a line a drop marker runs along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DropEdge {
  Top,
  Bottom,
}

impl Render for DragPreview {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let colors = Colors::active(cx);
    let lead = px(GRAB_LEAD);
    let left = if self.grab > lead {
      self.grab - lead
    } else {
      px(0.0)
    };
    div().pl(left).child(
      div()
        .h(px(ROW_HEIGHT))
        .max_w(px(SIDEBAR_WIDTH))
        .px_2()
        .flex()
        .items_center()
        .rounded_lg()
        .bg(colors.surface_raised)
        .border_1()
        .border_color(colors.surface_hover_strong)
        .opacity(0.85)
        .text_size(px(ROW_TEXT_SIZE))
        .text_color(colors.text)
        .child(div().min_w_0().truncate().child(self.name.clone())),
    )
  }
}

impl Workspace {
  /// Aims the drag by where the pointer is on the line `over`: how far down it, and which line
  /// it is. Nothing while the pointer is elsewhere, since every line hears every move.
  pub(super) fn aim_over(
    &mut self,
    over: &SidebarKey,
    event: &DragMoveEvent<DraggedLine>,
    cx: &mut Context<Self>,
  ) {
    if !self.points_at(event) {
      return;
    }
    let (bounds, pointer) = (event.bounds, event.event.position);
    let y = (pointer.y - bounds.top()) / bounds.size.height;
    let dragged = event.drag(cx).key.clone();
    let items = sidebar_items(&self.layout, &self.terminals);
    let spot = drop_spot(&items, &dragged, Hover { line: over, y });
    self.aim(spot, cx);
  }

  /// Aims the drag at `spot` while the pointer is on the element `event` is for, when letting go
  /// there would change something.
  pub(super) fn aim_at(
    &mut self,
    spot: DropSpot,
    event: &DragMoveEvent<DraggedLine>,
    cx: &mut Context<Self>,
  ) {
    if !self.points_at(event) {
      return;
    }
    let dragged = event.drag(cx).key.clone();
    let items = sidebar_items(&self.layout, &self.terminals);
    let spot = dropped(&items, &dragged, &spot).map(|_| spot);
    self.aim(spot, cx);
  }

  /// Clears the aim while the pointer is on the element `event` is for, one a drop can't land on,
  /// so the marker never stays behind on a line the pointer has left.
  pub(super) fn aim_nowhere(&mut self, event: &DragMoveEvent<DraggedLine>, cx: &mut Context<Self>) {
    if self.points_at(event) {
      self.aim(None, cx);
    }
  }

  /// Whether the pointer is on the element `event` is for, where it can be seen. A line scrolled
  /// out of view keeps its bounds under the title strip or past the window's edge, and must not
  /// take the aim there.
  fn points_at(&self, event: &DragMoveEvent<DraggedLine>) -> bool {
    let pointer = event.event.position;
    event.bounds.contains(&pointer) && self.sidebar_scroll.bounds().contains(&pointer)
  }

  /// Sets where the drag would land. It redraws only when that changed, since the pointer moves
  /// all the time.
  pub(super) fn aim(&mut self, spot: Option<DropSpot>, cx: &mut Context<Self>) {
    if self.drop_spot != spot {
      self.drop_spot = spot;
      cx.notify();
    }
  }

  /// Lets go of the dragged line where it aims. Aimed nowhere, nothing moves.
  pub(super) fn drop_line(
    &mut self,
    dragged: &SidebarKey,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(spot) = self.drop_spot.take() else {
      return;
    };
    if self.edit_sidebar(|items| dropped(items, dragged, &spot)) {
      self.changed(window, cx);
    } else {
      cx.notify();
    }
  }

  /// Where the drag would land, while one is on. A drag let go where nothing takes it ends
  /// without telling the sidebar, so its last spot is never shown after it.
  pub(super) fn shown_drop_spot(&self, cx: &App) -> Option<&DropSpot> {
    self.drop_spot.as_ref().filter(|_| cx.has_active_drag())
  }
}

/// The copy that follows the pointer while `line` is dragged, grabbed at `grab` from the line's
/// top-left corner.
pub(super) fn drag_preview(
  line: &DraggedLine,
  grab: Point<Pixels>,
  _: &mut Window,
  cx: &mut App,
) -> Entity<DragPreview> {
  cx.new(|_| DragPreview {
    name: line.name.clone(),
    grab: grab.x,
  })
}

/// The 2 px line that shows where a drop lands, along one edge of the line it is aimed against.
/// It starts where the line's icon does, after the indent inside a folder, and is drawn over the
/// line so nothing moves.
pub(super) fn drop_marker(edge: DropEdge, in_folder: bool, colors: Colors) -> AnyElement {
  div()
    .absolute()
    .left_0()
    .right_0()
    .map(|marker| match edge {
      DropEdge::Top => marker.top_0(),
      DropEdge::Bottom => marker.bottom_0(),
    })
    .h(px(DROP_MARKER_HEIGHT))
    .px_2()
    .flex()
    .gap_2()
    .when(in_folder, |marker| marker.child(sidebar::indent()))
    .child(
      div()
        .flex_1()
        .h_full()
        .rounded_full()
        .bg(colors.border_focused),
    )
    .into_any_element()
}

/// The outline a folder's line shows while a session dragged over it would go into the folder.
/// It is drawn over the line, so nothing moves.
pub(super) fn drop_outline(colors: Colors) -> AnyElement {
  div()
    .absolute()
    .inset_0()
    .rounded_lg()
    .border_1()
    .border_color(colors.border_focused)
    .into_any_element()
}
