//! The sidebar's context menus: a session's, a folder's and the empty sidebar's.

use gpui::ClickEvent;

use super::*;

/// The least height a menu is capped to, so a tiny window still shows a few items to scroll.
const MENU_MIN_MAX_HEIGHT: f32 = 120.0;

/// The tallest a sidebar menu may be in a window of this height; taller menus scroll.
fn menu_max_height(window_height: Pixels) -> Pixels {
  px((f32::from(window_height) - 2.0 * MENU_WINDOW_MARGIN).max(MENU_MIN_MAX_HEIGHT))
}

impl Workspace {
  /// The menu for the sidebar line or space it was opened on. `items` is this frame's sidebar.
  pub(super) fn render_menu(
    &self,
    menu: &SidebarMenu,
    items: &[SidebarItem],
    window_height: Pixels,
    cx: &mut Context<Self>,
  ) -> impl IntoElement {
    let colors = Colors::active(cx);
    let menu_colors = menu_colors(colors);
    let entries = match &menu.target {
      MenuTarget::Line(SidebarKey::Row(row)) => self.session_menu(items, row, menu_colors, cx),
      MenuTarget::Line(SidebarKey::Folder(id)) => self.folder_menu(items, *id, menu_colors, cx),
      MenuTarget::Sidebar => vec![
        menu_item(
          "new-folder",
          "New folder",
          self.shortcut(Action::NewFolder),
          true,
          menu_colors,
        )
        .on_click(menu_edit(
          |workspace, window, cx| workspace.new_folder(window, cx),
          cx,
        ))
        .into_any_element(),
      ],
    };
    div()
      .id("sidebar-menu")
      .min_w(px(MENU_MIN_WIDTH))
      .max_w(px(MENU_MAX_WIDTH))
      .max_h(menu_max_height(window_height))
      .overflow_y_scroll()
      .py_1()
      .rounded_lg()
      .border_1()
      .border_color(colors.surface_hover_strong)
      .bg(colors.surface_raised)
      .shadow_lg()
      .text_size(px(ROW_TEXT_SIZE))
      .text_color(colors.text)
      .occlude()
      .on_mouse_down_out(cx.listener(|workspace, _, _, cx| {
        workspace.menu = None;
        cx.notify();
      }))
      .children(entries)
  }

  /// A session row's items: rename, move up and down, move into or out of folders, and close.
  fn session_menu(
    &self,
    items: &[SidebarItem],
    row: &str,
    colors: MenuColors,
    cx: &mut Context<Self>,
  ) -> Vec<AnyElement> {
    let can_move_up = moved_row(items, row, Direction::Up).is_some();
    let can_move_down = moved_row(items, row, Direction::Down).is_some();
    let entry = find_row(items, row);
    let noun = match entry {
      Some(Entry::Loose(_)) => "terminal",
      _ => "tab",
    };
    // The shortcuts act on the active tab, so only that tab's row shows them.
    let is_active_tab = matches!(
      entry,
      Some(Entry::Tab { index, .. }) if *index == self.layout.active_tab
    );
    let shortcut = |action| is_active_tab.then(|| self.shortcut(action)).flatten();
    let current_folder = folder_of(items, row);
    let row: Rc<str> = row.into();
    let rename_row = row.clone();
    let up_row = row.clone();
    let down_row = row.clone();
    let new_folder_row = row.clone();
    let remove_row = row.clone();
    let close_row = row.clone();
    let mut menu = vec![
      menu_item("rename", &format!("Rename {noun}…"), None, true, colors)
        .on_click(cx.listener(move |workspace, _, window, cx| {
          workspace.menu = None;
          workspace.start_rename(SidebarKey::Row(rename_row.to_string()), window, cx);
        }))
        .into_any_element(),
      menu_separator(colors).into_any_element(),
      menu_item(
        "up",
        &format!("Move {noun} up"),
        shortcut(Action::MoveTabUp),
        can_move_up,
        colors,
      )
      .when(can_move_up, |item| {
        item.on_click(menu_edit(
          move |workspace, _, _| workspace.move_row(&up_row, Direction::Up),
          cx,
        ))
      })
      .into_any_element(),
      menu_item(
        "down",
        &format!("Move {noun} down"),
        shortcut(Action::MoveTabDown),
        can_move_down,
        colors,
      )
      .when(can_move_down, |item| {
        item.on_click(menu_edit(
          move |workspace, _, _| workspace.move_row(&down_row, Direction::Down),
          cx,
        ))
      })
      .into_any_element(),
      menu_separator(colors).into_any_element(),
      menu_item(
        "move-to-new-folder",
        "Move to new folder…",
        shortcut(Action::MoveToNewFolder),
        true,
        colors,
      )
      .on_click(menu_edit(
        move |workspace, window, cx| workspace.move_to_new_folder(&new_folder_row, window, cx),
        cx,
      ))
      .into_any_element(),
    ];
    let other_folders = items.iter().filter_map(|item| match item {
      SidebarItem::Folder(folder) if Some(folder.id) != current_folder => Some(folder),
      _ => None,
    });
    for folder in other_folders {
      let id = folder.id;
      let moved = row.clone();
      menu.push(
        menu_item(
          &format!("move-to-{id}"),
          &format!("Move to {}", folder.name),
          None,
          true,
          colors,
        )
        .on_click(menu_edit(
          move |workspace, _, _| workspace.move_to_folder(&moved, id),
          cx,
        ))
        .into_any_element(),
      );
    }
    if current_folder.is_some() {
      menu.push(
        menu_item(
          "remove-from-folder",
          "Remove from folder",
          shortcut(Action::RemoveFromFolder),
          true,
          colors,
        )
        .on_click(menu_edit(
          move |workspace, _, _| workspace.remove_from_folder(&remove_row),
          cx,
        ))
        .into_any_element(),
      );
    }
    menu.push(menu_separator(colors).into_any_element());
    menu.push(
      menu_item(
        "close",
        &format!("Close {noun}"),
        shortcut(Action::CloseTab),
        true,
        colors,
      )
      .on_click(cx.listener(move |workspace, _, _, cx| {
        workspace.menu = None;
        workspace.close_row(&close_row, cx);
      }))
      .into_any_element(),
    );
    menu
  }

  /// A folder's items: rename, move up and down, and delete, which leaves its sessions where it
  /// was.
  fn folder_menu(
    &self,
    items: &[SidebarItem],
    id: FolderId,
    colors: MenuColors,
    cx: &mut Context<Self>,
  ) -> Vec<AnyElement> {
    let can_move_up = moved_folder(items, id, Direction::Up).is_some();
    let can_move_down = moved_folder(items, id, Direction::Down).is_some();
    vec![
      menu_item("rename", "Rename folder…", None, true, colors)
        .on_click(cx.listener(move |workspace, _, window, cx| {
          workspace.menu = None;
          workspace.start_rename(SidebarKey::Folder(id), window, cx);
        }))
        .into_any_element(),
      menu_separator(colors).into_any_element(),
      menu_item("up", "Move folder up", None, can_move_up, colors)
        .when(can_move_up, |item| {
          item.on_click(menu_edit(
            move |workspace, _, _| workspace.move_folder(id, Direction::Up),
            cx,
          ))
        })
        .into_any_element(),
      menu_item("down", "Move folder down", None, can_move_down, colors)
        .when(can_move_down, |item| {
          item.on_click(menu_edit(
            move |workspace, _, _| workspace.move_folder(id, Direction::Down),
            cx,
          ))
        })
        .into_any_element(),
      menu_separator(colors).into_any_element(),
      menu_item("delete", "Delete folder", None, true, colors)
        .on_click(menu_edit(
          move |workspace, _, _| workspace.layout.delete_folder(id),
          cx,
        ))
        .into_any_element(),
    ]
  }
}

/// A menu item's click: closes the menu, makes the edit, then runs `changed`, which saves the
/// layout and, while a rename the edit started is on, leaves the keyboard with its field.
fn menu_edit(
  edit: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
  cx: &Context<Workspace>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
  cx.listener(move |workspace, _: &ClickEvent, window, cx| {
    workspace.menu = None;
    edit(workspace, window, cx);
    workspace.changed(window, cx);
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn menu_max_height_leaves_a_margin_on_both_sides() {
    assert_eq!(menu_max_height(px(800.0)), px(768.0));
  }

  #[test]
  fn menu_max_height_never_drops_below_the_floor() {
    assert_eq!(menu_max_height(px(50.0)), px(MENU_MIN_MAX_HEIGHT));
    assert_eq!(menu_max_height(px(0.0)), px(MENU_MIN_MAX_HEIGHT));
  }
}
