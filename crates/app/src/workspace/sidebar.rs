//! The sidebar: its rows and folders, the context menus, renaming and reordering.

use gpui::FontWeight;

use super::*;

/// The name a folder starts with, until its rename ends.
const NEW_FOLDER_NAME: &str = "New folder";

/// What a row does when opened, with the workspace it acts on.
type OnOpen = Box<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

/// One sidebar line's content, a row's or a folder's: what `sidebar_row` draws.
struct SidebarRow {
  key: SidebarKey,
  icon: AnyElement,
  name: String,
  /// `None` keeps the dot's room empty.
  dot: Option<Indicator>,
  /// How many rows need a person, shown in the folder line's close slot as a second cue besides
  /// the dot's color. Only folders set it.
  blocked: Option<String>,
  is_active: bool,
  is_live: bool,
  /// Indents the row under its folder's name.
  in_folder: bool,
  /// What clicking the row does.
  on_open: Option<OnOpen>,
}

pub(super) fn indicator_color(indicator: Indicator, colors: Colors) -> Hsla {
  match indicator {
    Indicator::Quiet => colors.status_quiet,
    Indicator::Working => colors.status_working,
    Indicator::NeedsYou => colors.status_blocked,
    Indicator::Done => colors.status_done,
    Indicator::Finished => colors.status_exited,
  }
}

/// The sidebar's lines on screen: top-level rows, folders, and the rows of open folders.
pub(super) fn visible_keys(items: &[SidebarItem]) -> HashSet<SidebarKey> {
  let mut keys = HashSet::new();
  for item in items {
    match item {
      SidebarItem::Row(entry) => keys.extend(row_key(entry)),
      SidebarItem::Folder(folder) => {
        keys.insert(SidebarKey::Folder(folder.id));
        if !folder.collapsed {
          keys.extend(folder.rows.iter().filter_map(row_key));
        }
      }
    }
  }
  keys
}

impl Workspace {
  /// The active tab's row: its first terminal.
  pub(super) fn active_row(&self) -> Option<String> {
    let tab = self.layout.active()?;
    tab.root.leaves().first().map(|first| first.to_string())
  }

  /// The row a terminal is in, at the top level or in a folder.
  pub(super) fn find_row(&self, row: &str) -> Option<Entry> {
    find_row(&sidebar_items(&self.layout, &self.terminals), row).cloned()
  }

  /// The folder the active tab is in, if any.
  pub(super) fn active_folder(&self) -> Option<FolderId> {
    let row = self.active_row()?;
    folder_of(&sidebar_items(&self.layout, &self.terminals), &row)
  }

  /// Moves a row one step, into and out of folders as it meets their edges; nothing when it
  /// can't move.
  pub(super) fn move_row(&mut self, row: &str, direction: Direction) {
    self.edit_sidebar(|items| moved_row(items, row, direction));
  }

  /// Moves a row into a new folder and starts naming the folder. The folder takes the row's
  /// place, or goes right after its folder when the row is already in one.
  pub(super) fn move_to_new_folder(
    &mut self,
    row: &str,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let id = self.layout.next_folder_id();
    if self.edit_sidebar(|items| moved_to_new_folder(items, row, id, NEW_FOLDER_NAME)) {
      self.start_rename(SidebarKey::Folder(id), window, cx);
    }
  }

  /// Adds an empty folder at the end of the sidebar, and starts naming it.
  pub(super) fn new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let id = self.layout.next_folder_id();
    self.edit_sidebar(|items| Some(with_new_folder(items, id, NEW_FOLDER_NAME)));
    self.start_rename(SidebarKey::Folder(id), window, cx);
  }

  /// Moves a row to the end of a folder, and opens it.
  pub(super) fn move_to_folder(&mut self, row: &str, id: FolderId) {
    self.edit_sidebar(|items| moved_to_folder(items, row, id));
  }

  /// Moves a row out of its folder, to right below it.
  pub(super) fn remove_from_folder(&mut self, row: &str) {
    self.edit_sidebar(|items| removed_from_folder(items, row));
  }

  /// Swaps a folder with the line next to it at the top level.
  pub(super) fn move_folder(&mut self, id: FolderId, direction: Direction) {
    self.edit_sidebar(|items| moved_folder(items, id, direction));
  }

  /// Lays out the sidebar as `edit` rearranges it; says whether the edit could happen. The
  /// caller runs `changed`, which saves it.
  pub(super) fn edit_sidebar(
    &mut self,
    edit: impl FnOnce(&[SidebarItem]) -> Option<Vec<SidebarItem>>,
  ) -> bool {
    let items = sidebar_items(&self.layout, &self.terminals);
    let Some(items) = edit(&items) else {
      return false;
    };
    self.layout.arrange(&items);
    true
  }

  /// Ends every terminal in the row: a tab's panes, or the one loose terminal.
  pub(super) fn close_row(&mut self, row: &str, cx: &mut Context<Self>) {
    if let Some(entry) = self.find_row(row) {
      for address in entry.terminals() {
        self.close_address(address);
      }
    }
    cx.notify();
  }

  /// Puts a field over a row's or a folder's name, starting from that name. The field keeps the
  /// keyboard until the rename ends, through any `changed` in between.
  pub(super) fn start_rename(
    &mut self,
    key: SidebarKey,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let text = match &key {
      SidebarKey::Row(row) => self
        .find_row(row)
        .map(|entry| entry_label(&self.layout, &self.terminals, &entry)),
      SidebarKey::Folder(id) => self.layout.folder(*id).map(|folder| folder.name.clone()),
    };
    let Some(text) = text else {
      return;
    };
    // The sidebar's own font, not the terminal's.
    let family = window.text_style().font_family;
    let colors = Colors::active(cx);
    let field_colors = TextFieldColors {
      text: colors.text,
      selection: colors.selection,
      cursor: colors.text,
    };
    let field = cx.new(|cx| TextField::new(&text, family, field_colors, window, cx));
    let events = cx.subscribe_in(&field, window, |workspace, _, event, window, cx| {
      let name = match event {
        FieldEvent::Submit(name) => Some(name.clone()),
        FieldEvent::Cancel => None,
      };
      workspace.finish_rename(name, window, cx);
    });
    self.renaming = Some(Renaming {
      key,
      field,
      _events: events,
    });
    cx.notify();
  }

  /// Ends the rename, keeping `name` if there is one, and hands the keyboard back to the pane.
  pub(super) fn finish_rename(
    &mut self,
    name: Option<String>,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let Some(renaming) = self.renaming.take() else {
      return;
    };
    if let Some(name) = name {
      match renaming.key {
        SidebarKey::Row(row) => {
          if let Some(entry) = self.find_row(&row) {
            self.layout.rename(&entry, &name);
          }
        }
        SidebarKey::Folder(id) => self.layout.rename_folder(id, &name),
      }
    }
    self.changed(window, cx);
  }

  /// Opens a sidebar menu at `position`, fading in.
  fn open_menu(&mut self, position: Point<Pixels>, target: MenuTarget, cx: &mut Context<Self>) {
    self.menu = Some(SidebarMenu {
      position,
      target,
      shown: Tween::new(0.0, 1.0, Instant::now(), motion::duration()),
    });
    cx.notify();
  }

  /// Draws the parts every sidebar line shares, with its menu and its name to edit. Only a row
  /// has a close button; a folder's line keeps its room, so the dots line up.
  fn sidebar_row(&self, content: SidebarRow, cx: &mut Context<Self>) -> AnyElement {
    let colors = Colors::active(cx);
    let SidebarRow {
      key,
      icon,
      name,
      dot,
      blocked,
      is_active,
      is_live,
      in_folder,
      on_open,
    } = content;
    let (id, row) = match &key {
      SidebarKey::Row(row) => (format!("row-{row}"), Some(row.clone())),
      SidebarKey::Folder(folder) => (format!("folder-{folder}"), None),
    };
    let dim = colors.text_muted;
    let renaming = self
      .renaming
      .as_ref()
      .filter(|renaming| renaming.key == key);
    // The line being renamed stays put, so the mouse can select text in its field.
    let dragged = renaming.is_none().then(|| DraggedLine {
      key: key.clone(),
      name: name.clone().into(),
    });
    let marker = match self.shown_drop_spot(cx) {
      Some(DropSpot::Before(line)) if *line == key => {
        Some(drop_marker(DropEdge::Top, in_folder, colors))
      }
      Some(DropSpot::After(line)) if *line == key => {
        Some(drop_marker(DropEdge::Bottom, in_folder, colors))
      }
      Some(DropSpot::Into(folder)) if key == SidebarKey::Folder(*folder) => {
        Some(drop_outline(colors))
      }
      _ => None,
    };
    let over = key.clone();
    let label = match renaming {
      Some(renaming) => {
        let field = renaming.field.clone();
        div()
          .on_mouse_down_out(cx.listener(move |workspace, _, window, cx| {
            let name = field.read(cx).text().to_string();
            workspace.finish_rename(Some(name), window, cx);
          }))
          .flex_1()
          .min_w_0()
          .h(px(22.0))
          .px_1()
          .rounded_sm()
          .border_1()
          .border_color(colors.border_focused)
          .bg(colors.surface)
          .child(renaming.field.clone())
      }
      None => div()
        .flex_1()
        .min_w_0()
        .truncate()
        .when(!is_live, |name| name.text_color(dim))
        .child(name),
    };
    let close_slot = div()
      .h(px(20.0))
      .min_w(px(20.0))
      .flex()
      .items_center()
      .justify_center()
      .rounded_md()
      .text_color(dim);
    let close = match row {
      Some(closing) => close_slot
        .id(SharedString::from(format!("close-{closing}")))
        .hover(|button| button.bg(colors.surface_hover_strong))
        // A live row shows its close button on hover only; a finished one always offers rm.
        .when(is_live, |button| {
          button
            .opacity(0.0)
            .group_hover("row", |button| button.opacity(1.0))
            .child(close_icon(dim))
        })
        .when(!is_live, |button| button.px_1().child("rm"))
        .on_click(cx.listener(move |workspace, _, _, cx| {
          cx.stop_propagation();
          workspace.close_row(&closing, cx);
        }))
        .into_any_element(),
      None => close_slot
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(colors.status_blocked)
        .children(blocked)
        .into_any_element(),
    };
    sidebar_line()
      .id(SharedString::from(id))
      .relative()
      .group("row")
      .rounded_lg()
      .when(is_active, |line| line.bg(colors.surface_active))
      .hover(|line| line.bg(colors.surface_hover))
      .when_some(on_open, |line, on_open| {
        line.on_click(cx.listener(move |workspace, _, window, cx| on_open(workspace, window, cx)))
      })
      .when_some(dragged, |line, dragged| line.on_drag(dragged, drag_preview))
      .on_drag_move(cx.listener(
        move |workspace, event: &DragMoveEvent<DraggedLine>, _, cx| {
          workspace.aim_over(&over, event, cx);
        },
      ))
      .on_mouse_down(
        MouseButton::Right,
        cx.listener(move |workspace, event: &MouseDownEvent, _, cx| {
          // This line's menu only: the sidebar's own is for the space around the lines.
          cx.stop_propagation();
          workspace.open_menu(event.position, MenuTarget::Line(key.clone()), cx);
        }),
      )
      .when(in_folder, |line| line.child(indent()))
      .child(icon)
      .child(label)
      .child(
        div()
          .size(px(6.0))
          .flex_none()
          .rounded_full()
          .when_some(dot, |slot, dot| slot.bg(indicator_color(dot, colors))),
      )
      .child(close)
      .children(marker)
      .into_any_element()
  }

  /// The dot a row shows; see `row_indicator`.
  fn row_dot(&self, entry: &Entry) -> Indicator {
    row_indicator(entry, &self.terminals)
  }

  pub(super) fn render_tab_row(
    &self,
    entry: &Entry,
    index: usize,
    in_folder: bool,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let colors = Colors::active(cx);
    let members = entry.terminals();
    let icon = if members.len() > 1 {
      stack_icon(StackColors {
        shell: shell_colors(colors),
        behind: colors.border_focused,
        outline: colors.surface,
      })
    } else {
      shell_icon(shell_colors(colors)).into_any_element()
    };
    let content = SidebarRow {
      key: SidebarKey::Row(
        members
          .first()
          .map(|first| first.to_string())
          .unwrap_or_default(),
      ),
      icon,
      name: entry_label(&self.layout, &self.terminals, entry),
      dot: Some(self.row_dot(entry)),
      blocked: None,
      is_active: index == self.layout.active_tab,
      is_live: true,
      in_folder,
      on_open: Some(Box::new(move |workspace, window, cx| {
        workspace.layout.select_tab(index);
        workspace.changed(window, cx);
      })),
    };
    self.sidebar_row(content, cx)
  }

  pub(super) fn render_loose_row(
    &self,
    entry: &Entry,
    terminal: &Terminal,
    in_folder: bool,
    cx: &mut Context<Self>,
  ) -> AnyElement {
    let colors = Colors::active(cx);
    let address = terminal.address.clone();
    let is_live = terminal.state == TerminalState::Live;
    let content = SidebarRow {
      key: SidebarKey::Row(address.clone()),
      icon: shell_icon(shell_colors(colors)).into_any_element(),
      name: entry_label(&self.layout, &self.terminals, entry),
      dot: Some(self.row_dot(entry)),
      blocked: None,
      is_active: false,
      is_live,
      in_folder,
      on_open: is_live.then(|| -> OnOpen {
        Box::new(move |workspace, window, cx| {
          workspace.layout.show(&address);
          workspace.changed(window, cx);
        })
      }),
    };
    self.sidebar_row(content, cx)
  }

  /// A folder's line; a click opens or collapses it. Collapsed, it stands in for its rows: it
  /// shows their most urgent dot (none when empty), and the active tab's highlight when that tab is one of them.
  fn render_folder_row(&self, folder: &SidebarFolder, cx: &mut Context<Self>) -> AnyElement {
    let colors = Colors::active(cx);
    let icon = if folder.collapsed {
      ICON_FOLDER
    } else {
      ICON_FOLDER_OPENED
    };
    let (dot, blocked) = if folder.collapsed && !folder.rows.is_empty() {
      let dots = || folder.rows.iter().map(|entry| self.row_dot(entry));
      (Some(most_urgent(dots())), blocked_label(dots()))
    } else {
      (None, None)
    };
    let holds_active_tab = folder
      .rows
      .iter()
      .any(|entry| matches!(entry, Entry::Tab { index, .. } if *index == self.layout.active_tab));
    let id = folder.id;
    let content = SidebarRow {
      key: SidebarKey::Folder(id),
      icon: glyph(icon, colors),
      name: folder.name.clone(),
      dot,
      blocked,
      is_active: folder.collapsed && holds_active_tab,
      is_live: true,
      in_folder: false,
      on_open: Some(Box::new(move |workspace, window, cx| {
        workspace.layout.toggle_folder(id);
        workspace.changed(window, cx);
      })),
    };
    self.sidebar_row(content, cx)
  }

  pub(super) fn sidebar_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let colors = Colors::active(cx);
    chrome_button("toggle-sidebar", colors)
      .child(sidebar_icon(colors.text_muted))
      .on_click(cx.listener(|workspace, _, _, cx| {
        workspace.toggle_sidebar();
        cx.notify();
      }))
  }

  /// Starts a shell. It belongs to the sidebar and goes with it.
  fn new_shell_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
    let colors = Colors::active(cx);
    chrome_button("new-shell", colors)
      .child(plus_icon(colors.text_muted))
      .on_click(cx.listener(|workspace, _, window, cx| {
        workspace.run_action(Action::NewTab, window, cx);
      }))
  }

  pub(super) fn render_sidebar(
    &self,
    items: &[SidebarItem],
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let colors = Colors::active(cx);
    // Leaves room for the macOS traffic lights, which sit over this strip.
    let titlebar = div()
      .h(px(TITLEBAR_HEIGHT))
      .flex_none()
      .flex()
      .items_center()
      .justify_end()
      .pr_2()
      // Leaves the fixed sidebar toggle its room.
      .pl(px(TOGGLE_LEFT + TITLEBAR_BUTTON + TOGGLE_GAP))
      .pt(px(TITLEBAR_NUDGE))
      .on_mouse_down(MouseButton::Left, on_titlebar_mouse_down)
      .child(self.new_shell_button(cx));
    let label = div()
      .px_4()
      .pt_2()
      .pb_1()
      .text_size(px(11.0))
      .text_color(colors.text_muted)
      .on_drag_move(
        cx.listener(|workspace, event: &DragMoveEvent<DraggedLine>, _, cx| {
          workspace.aim_nowhere(event, cx);
        }),
      )
      .child("TERMINALS");
    let templates: Vec<AnyElement> = self
      .templates
      .iter()
      .map(|template| {
        let name = template.name.clone();
        div()
          .id(SharedString::from(format!("template-{name}")))
          .mx_2()
          .px_2()
          .h(px(28.0))
          .flex()
          .items_center()
          .rounded_lg()
          .text_color(colors.text_muted)
          .hover(|button| button.bg(colors.surface_hover))
          .child(format!("+ {name}"))
          .on_click(cx.listener(move |workspace, _, window, cx| {
            if workspace.open_tab(vec![name.clone()], None, cx) {
              workspace.changed(window, cx);
            }
          }))
          .into_any_element()
      })
      .collect();
    let now = Instant::now();
    let mut lines = Vec::new();
    for item in items {
      match item {
        SidebarItem::Row(entry) => lines.extend(self.render_entry(entry, false, now, cx)),
        SidebarItem::Folder(folder) => {
          let key = SidebarKey::Folder(folder.id);
          let line = self.render_folder_row(folder, cx);
          lines.push(self.grow_in(&key, line, now));
          if folder.collapsed {
            continue;
          }
          // Grows in with its folder, so a folder just made comes in as one.
          if folder.rows.is_empty() {
            lines.push(self.grow_in(&key, self.no_sessions(folder.id, cx), now));
          }
          for entry in &folder.rows {
            lines.extend(self.render_entry(entry, true, now, cx));
          }
        }
      }
    }
    let end_marker = matches!(self.shown_drop_spot(cx), Some(DropSpot::End)).then(|| {
      // Inset like a line, so this marker lines up with a top-level line's.
      div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .mx_2()
        .child(drop_marker(DropEdge::Top, false, colors))
    });
    // Below the lines: the templates, then the rest of the sidebar's height. A session dropped
    // here goes to the end of the top level, outside every folder. It grows into free space but
    // never shrinks below its templates: the scroll area measures only its own children, so a
    // template squeezed out of the tail could not be scrolled to.
    let tail = div()
      .relative()
      .flex_grow()
      .flex_shrink_0()
      .min_h(px(ROW_HEIGHT))
      .flex()
      .flex_col()
      .gap(px(LINE_GAP))
      .on_drag_move(
        cx.listener(|workspace, event: &DragMoveEvent<DraggedLine>, _, cx| {
          workspace.aim_at(DropSpot::End, event, cx);
        }),
      )
      .children(templates)
      .children(end_marker);
    div()
      .w(px(self.sidebar_drawn))
      .h_full()
      .flex_none()
      .flex()
      .flex_col()
      .text_color(colors.text)
      .text_size(px(ROW_TEXT_SIZE))
      .child(titlebar)
      .child(
        div()
          .id("sidebar")
          .flex_1()
          .min_h_0()
          .flex()
          .flex_col()
          .gap(px(LINE_GAP))
          .overflow_y_scroll()
          .track_scroll(&self.sidebar_scroll)
          // Every drag starts with a press here: no spot a drag let go elsewhere left behind can
          // carry over to the next one.
          .on_mouse_down(
            MouseButton::Left,
            cx.listener(|workspace, _: &MouseDownEvent, _, _| workspace.drop_spot = None),
          )
          // The heading and the space around the lines; a line opens its own menu instead.
          .on_mouse_down(
            MouseButton::Right,
            cx.listener(|workspace, event: &MouseDownEvent, _, cx| {
              workspace.open_menu(event.position, MenuTarget::Sidebar, cx);
            }),
          )
          // Outside the sidebar a drag aims nowhere, so letting go there moves nothing.
          .on_drag_move(
            cx.listener(|workspace, event: &DragMoveEvent<DraggedLine>, _, cx| {
              if !event.bounds.contains(&event.event.position) {
                workspace.aim(None, cx);
              }
            }),
          )
          .on_drop(cx.listener(|workspace, line: &DraggedLine, window, cx| {
            workspace.drop_line(&line.key, window, cx);
          }))
          .child(label)
          .children(lines)
          .child(tail),
      )
  }

  /// A tab's or a loose terminal's row, growing in while new; `None` for a loose terminal the
  /// core no longer has.
  fn render_entry(
    &self,
    entry: &Entry,
    in_folder: bool,
    now: Instant,
    cx: &mut Context<Self>,
  ) -> Option<AnyElement> {
    let row = match entry {
      Entry::Tab { index, .. } => self.render_tab_row(entry, *index, in_folder, cx),
      Entry::Loose(address) => {
        let terminal = self.terminals.get(address)?.clone();
        self.render_loose_row(entry, &terminal, in_folder, cx)
      }
    };
    Some(self.grow_in(&row_key(entry)?, row, now))
  }

  /// A line new on the sidebar grows to its height and fades in.
  fn grow_in(&self, key: &SidebarKey, line: AnyElement, now: Instant) -> AnyElement {
    let grown = self
      .row_motion
      .get(key)
      .map_or(1.0, |tween| tween.value(now));
    div()
      .flex_none()
      .h(px(ROW_HEIGHT * grown))
      .opacity(grown)
      .overflow_hidden()
      .child(line)
      .into_any_element()
  }

  /// What an open folder with no rows shows under its name. It isn't a row, so it has no hover
  /// and no click, but a session dropped on it goes into the folder.
  fn no_sessions(&self, id: FolderId, cx: &mut Context<Self>) -> AnyElement {
    let colors = Colors::active(cx);
    sidebar_line()
      .flex_none()
      .text_color(colors.text_muted)
      .on_drag_move(cx.listener(
        move |workspace, event: &DragMoveEvent<DraggedLine>, _, cx| {
          workspace.aim_at(DropSpot::Into(id), event, cx);
        },
      ))
      .child(indent())
      .child("No sessions")
      .into_any_element()
  }
}

/// A sidebar row is known by its first terminal.
fn row_key(entry: &Entry) -> Option<SidebarKey> {
  let first = entry.terminals().first()?.to_string();
  Some(SidebarKey::Row(first))
}

/// The frame of every sidebar line, so their icons and names line up.
fn sidebar_line() -> gpui::Div {
  div()
    .mx_2()
    .px_2()
    .h(px(ROW_HEIGHT))
    .flex()
    .items_center()
    .gap_2()
}

/// An empty icon's room before a nested line's content. With the line's gap after it, the
/// content starts right under its folder's name.
pub(super) fn indent() -> gpui::Div {
  div().size(px(ICON_BOX)).flex_none()
}

/// The colors of a shell's icon in the sidebar.
fn shell_colors(colors: Colors) -> ShellColors {
  ShellColors {
    background: colors.surface_active,
    text: colors.text_muted,
  }
}
