//! Dragging a sidebar line: where the pointer aims it, and the move a drop makes.

use super::{SidebarFolder, SidebarItem, SidebarKey, folder_at, locate, taken};
use crate::layout::FolderId;

/// Where a dragged line lands. It is named by the line it is aimed against, so a drop still means
/// the same place when the sidebar changes during the drag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropSpot {
  /// Right before this line, at its level: in its folder, or at the top level.
  Before(SidebarKey),
  /// Right after this line, at its level.
  After(SidebarKey),
  /// At the end of this folder.
  Into(FolderId),
  /// At the end of the top level.
  End,
}

/// The line a drag is over, and how far down it the pointer is: 0 at its top edge, 1 at its
/// bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover<'a> {
  pub line: &'a SidebarKey,
  pub y: f32,
}

/// Where `dragged` lands if let go at `hover`. A row's line splits in halves, before and after
/// it, and so does any line under a dragged folder, except the lower half of an open folder's
/// line, which aims nowhere. A folder's line takes a dragged row into the folder, except at its
/// edges: its top quarter is before the folder, and its bottom quarter is right under it, which
/// is after a collapsed folder and first in an open one. `None` when letting go there would
/// change nothing.
pub fn drop_spot(
  items: &[SidebarItem],
  dragged: &SidebarKey,
  hover: Hover<'_>,
) -> Option<DropSpot> {
  let spot = match (dragged, hover.line) {
    (SidebarKey::Row(_), SidebarKey::Folder(id)) => folder_spot(items, *id, hover.y)?,
    (_, line) if hover.y < 0.5 => DropSpot::Before(line.clone()),
    // Right under an open folder's name are its rows, where a folder can't land; the spot after
    // the whole folder is the next top-level line's.
    (SidebarKey::Folder(_), SidebarKey::Folder(id)) if !folder(items, *id)?.collapsed => {
      return None;
    }
    (_, line) => DropSpot::After(line.clone()),
  };
  dropped(items, dragged, &spot).map(|_| spot)
}

/// Moves `dragged` to `spot`. A row moves whole, and opens a collapsed folder it lands in; a
/// folder moves only among the top-level lines. `None` when a line is gone, the spot names the
/// dragged line, a folder would go inside a folder, or nothing would change.
pub fn dropped(
  items: &[SidebarItem],
  dragged: &SidebarKey,
  spot: &DropSpot,
) -> Option<Vec<SidebarItem>> {
  let moved = match dragged {
    SidebarKey::Row(row) => dropped_row(items, row, spot),
    SidebarKey::Folder(id) => dropped_folder(items, *id, spot),
  }?;
  (moved.as_slice() != items).then_some(moved)
}

/// Where a dragged row aims over folder `id`'s line, `y` of the way down it.
fn folder_spot(items: &[SidebarItem], id: FolderId, y: f32) -> Option<DropSpot> {
  let folder = folder(items, id)?;
  if y < 0.25 {
    return Some(DropSpot::Before(SidebarKey::Folder(id)));
  }
  if y <= 0.75 {
    return Some(DropSpot::Into(id));
  }
  if folder.collapsed {
    return Some(DropSpot::After(SidebarKey::Folder(id)));
  }
  // Right under an open folder's line is its first row, or "No sessions" when it has none.
  let first = folder
    .rows
    .first()
    .and_then(|entry| entry.terminals().first().map(|address| address.to_string()));
  Some(match first {
    Some(address) => DropSpot::Before(SidebarKey::Row(address)),
    None => DropSpot::Into(id),
  })
}

fn dropped_row(items: &[SidebarItem], row: &str, spot: &DropSpot) -> Option<Vec<SidebarItem>> {
  let (mut items, entry) = taken(items, row)?;
  match spot {
    DropSpot::Before(line) | DropSpot::After(line) => {
      let after = usize::from(matches!(spot, DropSpot::After(_)));
      match line {
        SidebarKey::Folder(id) => {
          let at = folder_position(&items, *id)?;
          items.insert(at + after, SidebarItem::Row(entry));
        }
        SidebarKey::Row(next_to) => {
          let next_to = locate(&items, next_to)?;
          match next_to.inner {
            Some(at) => {
              let folder = folder_at(&mut items, next_to.top);
              folder.rows.insert(at + after, entry);
              folder.collapsed = false;
            }
            None => items.insert(next_to.top + after, SidebarItem::Row(entry)),
          }
        }
      }
    }
    DropSpot::Into(id) => {
      let at = folder_position(&items, *id)?;
      let folder = folder_at(&mut items, at);
      folder.rows.push(entry);
      folder.collapsed = false;
    }
    DropSpot::End => items.push(SidebarItem::Row(entry)),
  }
  Some(items)
}

fn dropped_folder(
  items: &[SidebarItem],
  id: FolderId,
  spot: &DropSpot,
) -> Option<Vec<SidebarItem>> {
  let from = folder_position(items, id)?;
  let mut items = items.to_vec();
  let folder = items.remove(from);
  let at = match spot {
    DropSpot::Before(line) | DropSpot::After(line) => {
      let next_to = match line {
        SidebarKey::Folder(other) => folder_position(&items, *other)?,
        SidebarKey::Row(row) => {
          let next_to = locate(&items, row)?;
          // A folder doesn't go inside a folder.
          if next_to.inner.is_some() {
            return None;
          }
          next_to.top
        }
      };
      next_to + usize::from(matches!(spot, DropSpot::After(_)))
    }
    DropSpot::Into(_) => return None,
    DropSpot::End => items.len(),
  };
  items.insert(at, folder);
  Some(items)
}

/// Folder `id`, as the sidebar shows it.
fn folder(items: &[SidebarItem], id: FolderId) -> Option<&SidebarFolder> {
  items.iter().find_map(|item| match item {
    SidebarItem::Folder(folder) if folder.id == id => Some(folder),
    _ => None,
  })
}

/// The top-level index of folder `id`.
fn folder_position(items: &[SidebarItem], id: FolderId) -> Option<usize> {
  items
    .iter()
    .position(|item| matches!(item, SidebarItem::Folder(folder) if folder.id == id))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Entry;
  use crate::layout::testing::id;
  use crate::sidebar::tests::{in_folder, row};

  fn key(address: &str) -> SidebarKey {
    SidebarKey::Row(address.into())
  }

  fn folder_key(number: u32) -> SidebarKey {
    SidebarKey::Folder(id(number))
  }

  fn over(line: &SidebarKey, y: f32) -> Hover<'_> {
    Hover { line, y }
  }

  #[test]
  fn a_row_dropped_next_to_a_row_lands_at_its_level() {
    let items = vec![row("t1"), in_folder(1, false, &["t2", "t3"]), row("t4")];
    assert_eq!(
      dropped(&items, &key("t4"), &DropSpot::Before(key("t3"))),
      Some(vec![row("t1"), in_folder(1, false, &["t2", "t4", "t3"])])
    );
    assert_eq!(
      dropped(&items, &key("t2"), &DropSpot::After(key("t4"))),
      Some(vec![
        row("t1"),
        in_folder(1, false, &["t3"]),
        row("t4"),
        row("t2")
      ])
    );
  }

  #[test]
  fn a_row_dropped_into_a_collapsed_folder_goes_last_and_opens_it() {
    let items = vec![row("t1"), in_folder(1, true, &["t2"])];
    assert_eq!(
      dropped(&items, &key("t1"), &DropSpot::Into(id(1))),
      Some(vec![in_folder(1, false, &["t2", "t1"])])
    );
  }

  #[test]
  fn a_row_dropped_around_a_folder_or_at_the_end_is_at_the_top_level() {
    let items = vec![in_folder(1, true, &["t1", "t2"]), row("t3")];
    assert_eq!(
      dropped(&items, &key("t3"), &DropSpot::Before(folder_key(1))),
      Some(vec![row("t3"), in_folder(1, true, &["t1", "t2"])])
    );
    assert_eq!(
      dropped(&items, &key("t1"), &DropSpot::After(folder_key(1))),
      Some(vec![in_folder(1, true, &["t2"]), row("t1"), row("t3")])
    );
    assert_eq!(
      dropped(&items, &key("t2"), &DropSpot::End),
      Some(vec![in_folder(1, true, &["t1"]), row("t3"), row("t2")])
    );
  }

  #[test]
  fn a_tab_moves_whole() {
    let tab = Entry::Tab {
      index: 0,
      terminals: vec!["t2".into(), "t3".into()],
    };
    let items = vec![row("t1"), SidebarItem::Row(tab.clone())];
    assert_eq!(
      dropped(&items, &key("t2"), &DropSpot::Before(key("t1"))),
      Some(vec![SidebarItem::Row(tab), row("t1")])
    );
  }

  #[test]
  fn a_folder_moves_only_among_top_level_lines() {
    let items = vec![
      row("t1"),
      in_folder(1, false, &["t2"]),
      in_folder(2, false, &[]),
    ];
    assert_eq!(
      dropped(&items, &folder_key(2), &DropSpot::Before(key("t1"))),
      Some(vec![
        in_folder(2, false, &[]),
        row("t1"),
        in_folder(1, false, &["t2"])
      ])
    );
    assert_eq!(
      dropped(&items, &folder_key(1), &DropSpot::After(folder_key(2))),
      Some(vec![
        row("t1"),
        in_folder(2, false, &[]),
        in_folder(1, false, &["t2"])
      ])
    );
    assert_eq!(
      dropped(&items, &folder_key(2), &DropSpot::Before(key("t2"))),
      None
    );
    assert_eq!(
      dropped(&items, &folder_key(2), &DropSpot::Into(id(1))),
      None
    );
  }

  #[test]
  fn a_drop_that_changes_nothing_is_none() {
    let items = vec![row("t1"), row("t2")];
    assert_eq!(
      dropped(&items, &key("t1"), &DropSpot::Before(key("t2"))),
      None
    );
    assert_eq!(
      dropped(&items, &key("t1"), &DropSpot::After(key("t1"))),
      None
    );
    assert_eq!(dropped(&items, &key("t2"), &DropSpot::End), None);
  }

  #[test]
  fn a_drop_naming_a_gone_line_changes_nothing() {
    let items = vec![row("t1"), in_folder(1, false, &["t2"])];
    assert_eq!(dropped(&items, &key("t9"), &DropSpot::End), None);
    assert_eq!(
      dropped(&items, &key("t1"), &DropSpot::Before(key("t9"))),
      None
    );
    assert_eq!(dropped(&items, &key("t1"), &DropSpot::Into(id(9))), None);
    assert_eq!(dropped(&items, &folder_key(9), &DropSpot::End), None);
  }

  #[test]
  fn the_halves_of_a_row_aim_before_and_after_it() {
    let items = vec![row("t1"), row("t2"), row("t3")];
    let t1 = key("t1");
    assert_eq!(
      drop_spot(&items, &t1, over(&key("t3"), 0.2)),
      Some(DropSpot::Before(key("t3")))
    );
    assert_eq!(
      drop_spot(&items, &t1, over(&key("t3"), 0.8)),
      Some(DropSpot::After(key("t3")))
    );
  }

  #[test]
  fn spots_that_change_nothing_aim_nowhere() {
    let items = vec![row("t1"), row("t2")];
    let t1 = key("t1");
    assert_eq!(drop_spot(&items, &t1, over(&t1, 0.8)), None);
    assert_eq!(drop_spot(&items, &t1, over(&key("t2"), 0.2)), None);
  }

  #[test]
  fn a_folder_line_takes_a_row_into_it_or_around_it() {
    let items = vec![
      in_folder(1, true, &["t1"]),
      in_folder(2, false, &["t2"]),
      in_folder(3, false, &[]),
      row("t4"),
    ];
    let t4 = key("t4");
    let aim = |number, y| drop_spot(&items, &t4, over(&folder_key(number), y));
    assert_eq!(aim(1, 0.1), Some(DropSpot::Before(folder_key(1))));
    assert_eq!(aim(1, 0.5), Some(DropSpot::Into(id(1))));
    assert_eq!(aim(1, 0.9), Some(DropSpot::After(folder_key(1))));
    assert_eq!(aim(2, 0.9), Some(DropSpot::Before(key("t2"))));
    assert_eq!(aim(3, 0.9), Some(DropSpot::Into(id(3))));
  }

  #[test]
  fn a_folder_aims_only_around_top_level_lines() {
    let items = vec![
      row("t1"),
      in_folder(1, false, &["t2"]),
      in_folder(2, false, &[]),
    ];
    let f2 = folder_key(2);
    assert_eq!(
      drop_spot(&items, &f2, over(&key("t1"), 0.2)),
      Some(DropSpot::Before(key("t1")))
    );
    assert_eq!(
      drop_spot(&items, &f2, over(&folder_key(1), 0.2)),
      Some(DropSpot::Before(folder_key(1)))
    );
    assert_eq!(drop_spot(&items, &f2, over(&key("t2"), 0.2)), None);
  }

  #[test]
  fn a_folder_aims_after_a_collapsed_folder_but_not_after_an_open_one() {
    // Right under an open folder's name are its rows, or "No sessions", where a folder can't
    // land: the spot after the whole folder is the next top-level line's.
    let f2 = folder_key(2);
    let aim = |folder| {
      let items = vec![folder, row("t4"), in_folder(2, true, &[])];
      drop_spot(&items, &f2, over(&folder_key(1), 0.8))
    };
    assert_eq!(aim(in_folder(1, false, &["t2"])), None);
    assert_eq!(aim(in_folder(1, false, &[])), None);
    assert_eq!(
      aim(in_folder(1, true, &["t2"])),
      Some(DropSpot::After(folder_key(1)))
    );
  }
}
