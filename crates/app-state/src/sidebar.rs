//! The sidebar as the person arranged it: rows at the top level and in folders.

use crate::layout::{FolderId, Layout, Place};
use crate::{Entry, Terminals};

mod drag;

pub use drag::{DropSpot, Hover, drop_spot, dropped};

/// A folder as the sidebar shows it, with its rows in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarFolder {
  pub id: FolderId,
  pub name: String,
  pub collapsed: bool,
  pub rows: Vec<Entry>,
}

/// One line at the sidebar's top level: a row, or a folder with its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarItem {
  Row(Entry),
  Folder(SidebarFolder),
}

/// A line of the sidebar: a row, known by its first terminal, or a folder, by its id. Rows
/// move and tabs renumber, so neither is known by its place.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SidebarKey {
  Row(String),
  Folder(FolderId),
}

/// The sidebar as shown: every tab, then every loose terminal that isn't leaving, each placed by
/// its best ranked terminal in the layout's order. Terminals the order doesn't list go last, in the
/// core's order, at the top level.
pub fn sidebar_items(layout: &Layout, terminals: &Terminals) -> Vec<SidebarItem> {
  let tabs = layout
    .tabs
    .iter()
    .enumerate()
    .map(|(index, tab)| Entry::Tab {
      index,
      terminals: tab.root.leaves().into_iter().map(str::to_string).collect(),
    });
  let shown = layout.shown();
  let loose = terminals
    .list()
    .iter()
    // One about to be removed would flash in the list until it is.
    .filter(|terminal| {
      !shown.contains(&terminal.address.as_str()) && !terminals.is_leaving(terminal)
    })
    .map(|terminal| Entry::Loose(terminal.address.clone()));
  let places = layout.places();
  let arranged = places.len();
  let rank = |address: &str| {
    let listed = places
      .iter()
      .enumerate()
      .find_map(|(index, place)| match place {
        Place::Terminal {
          address: known,
          folder,
        } if *known == address => Some((index, *folder)),
        _ => None,
      });
    listed.unwrap_or_else(|| {
      let core = terminals
        .list()
        .iter()
        .position(|terminal| terminal.address == address);
      (arranged + core.unwrap_or(usize::MAX - arranged), None)
    })
  };
  let mut rows: Vec<(usize, Option<FolderId>, Entry)> = tabs
    .chain(loose)
    .filter_map(|entry| {
      let (rank, folder) = entry
        .terminals()
        .into_iter()
        .map(rank)
        .min_by_key(|(rank, _)| *rank)?;
      Some((rank, folder, entry))
    })
    .collect();
  rows.sort_by_key(|(rank, ..)| *rank);

  let mut items: Vec<(usize, SidebarItem)> = Vec::new();
  for (index, place) in places.iter().enumerate() {
    if let Place::Folder(folder) = place {
      let rows = rows
        .iter()
        .filter(|(_, held, _)| *held == Some(folder.id))
        .map(|(.., entry)| entry.clone())
        .collect();
      items.push((
        index,
        SidebarItem::Folder(SidebarFolder {
          id: folder.id,
          name: folder.name.clone(),
          collapsed: folder.collapsed,
          rows,
        }),
      ));
    }
  }
  items.extend(
    rows
      .into_iter()
      .filter(|(_, folder, _)| folder.is_none())
      .map(|(rank, _, entry)| (rank, SidebarItem::Row(entry))),
  );
  items.sort_by_key(|(rank, _)| *rank);
  items.into_iter().map(|(_, item)| item).collect()
}

/// The row holding this terminal, at the top level or in a folder.
pub fn find_row<'a>(items: &'a [SidebarItem], terminal: &str) -> Option<&'a Entry> {
  let holds = |entry: &&Entry| entry.terminals().contains(&terminal);
  items.iter().find_map(|item| match item {
    SidebarItem::Row(entry) => Some(entry).filter(holds),
    SidebarItem::Folder(folder) => folder.rows.iter().find(holds),
  })
}

/// The folder holding this terminal's row; `None` at the top level or with no such row.
pub fn folder_of(items: &[SidebarItem], terminal: &str) -> Option<FolderId> {
  items.iter().find_map(|item| match item {
    SidebarItem::Folder(folder)
      if folder
        .rows
        .iter()
        .any(|entry| entry.terminals().contains(&terminal)) =>
    {
      Some(folder.id)
    }
    _ => None,
  })
}

/// Which way a row or a folder moves in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
  Up,
  Down,
}

/// Moves a row one step. It swaps with its neighbor, but crosses a folder's edge when it meets
/// one: it enters an open folder, leaves a folder at its first or last row, and jumps over a
/// collapsed one. A row moved within a collapsed folder opens it. `None` when it can't move.
pub fn moved_row(
  items: &[SidebarItem],
  row: &str,
  direction: Direction,
) -> Option<Vec<SidebarItem>> {
  let mut items = items.to_vec();
  let spot = locate(&items, row)?;
  let up = direction == Direction::Up;
  match spot.inner {
    Some(at) => {
      let rows = &mut folder_at(&mut items, spot.top).rows;
      let next = if up { at.checked_sub(1) } else { Some(at + 1) };
      match next.filter(|next| *next < rows.len()) {
        Some(next) => {
          rows.swap(at, next);
          folder_at(&mut items, spot.top).collapsed = false;
        }
        None => {
          let entry = rows.remove(at);
          items.insert(spot.top + usize::from(!up), SidebarItem::Row(entry));
        }
      }
    }
    None => {
      let next = if up {
        spot.top.checked_sub(1)
      } else {
        Some(spot.top + 1)
      };
      let next = next.filter(|next| *next < items.len())?;
      match &mut items[next] {
        SidebarItem::Folder(folder) if !folder.collapsed => {
          let SidebarItem::Row(entry) = items.remove(spot.top) else {
            return None;
          };
          let folder = folder_at(&mut items, next - usize::from(!up));
          if up {
            folder.rows.push(entry);
          } else {
            folder.rows.insert(0, entry);
          }
        }
        _ => items.swap(spot.top, next),
      }
    }
  }
  Some(items)
}

/// Swaps a folder with the top-level item next to it, a row or another folder.
pub fn moved_folder(
  items: &[SidebarItem],
  id: FolderId,
  direction: Direction,
) -> Option<Vec<SidebarItem>> {
  let at = items
    .iter()
    .position(|item| matches!(item, SidebarItem::Folder(folder) if folder.id == id))?;
  let next = match direction {
    Direction::Up => at.checked_sub(1)?,
    Direction::Down => at + 1,
  };
  (next < items.len()).then(|| {
    let mut items = items.to_vec();
    items.swap(at, next);
    items
  })
}

/// An empty open folder at the end of the top level.
pub fn with_new_folder(items: &[SidebarItem], id: FolderId, name: &str) -> Vec<SidebarItem> {
  let mut items = items.to_vec();
  items.push(new_folder(id, name, Vec::new()));
  items
}

/// Moves a row into a new open folder: in the row's place, or right after its old folder.
pub fn moved_to_new_folder(
  items: &[SidebarItem],
  row: &str,
  id: FolderId,
  name: &str,
) -> Option<Vec<SidebarItem>> {
  let spot = locate(items, row)?;
  let (mut items, entry) = taken(items, row)?;
  let at = spot.top + usize::from(spot.inner.is_some());
  items.insert(at, new_folder(id, name, vec![entry]));
  Some(items)
}

/// Moves a row to the end of a folder and opens it. `None` if it is already in that folder.
pub fn moved_to_folder(items: &[SidebarItem], row: &str, id: FolderId) -> Option<Vec<SidebarItem>> {
  if folder_of(items, row) == Some(id) {
    return None;
  }
  let (mut items, entry) = taken(items, row)?;
  let folder = items.iter_mut().find_map(|item| match item {
    SidebarItem::Folder(folder) if folder.id == id => Some(folder),
    _ => None,
  })?;
  folder.rows.push(entry);
  folder.collapsed = false;
  Some(items)
}

/// Moves a row out of its folder to the top level, right below it.
pub fn removed_from_folder(items: &[SidebarItem], row: &str) -> Option<Vec<SidebarItem>> {
  let spot = locate(items, row).filter(|spot| spot.inner.is_some())?;
  let (mut items, entry) = taken(items, row)?;
  items.insert(spot.top + 1, SidebarItem::Row(entry));
  Some(items)
}

/// Where a row sits: its top-level index, and its index in that folder if it is in one.
struct Spot {
  top: usize,
  inner: Option<usize>,
}

fn locate(items: &[SidebarItem], row: &str) -> Option<Spot> {
  let holds = |entry: &Entry| entry.terminals().contains(&row);
  items.iter().enumerate().find_map(|(top, item)| match item {
    SidebarItem::Row(entry) => holds(entry).then_some(Spot { top, inner: None }),
    SidebarItem::Folder(folder) => folder.rows.iter().position(holds).map(|at| Spot {
      top,
      inner: Some(at),
    }),
  })
}

/// A copy of `items` without the row, and the row itself.
fn taken(items: &[SidebarItem], row: &str) -> Option<(Vec<SidebarItem>, Entry)> {
  let spot = locate(items, row)?;
  let mut items = items.to_vec();
  let entry = match spot.inner {
    Some(at) => folder_at(&mut items, spot.top).rows.remove(at),
    None => match items.remove(spot.top) {
      SidebarItem::Row(entry) => entry,
      SidebarItem::Folder(_) => return None,
    },
  };
  Some((items, entry))
}

/// The folder at a top-level index that `locate` or a folder check already vouched for.
fn folder_at(items: &mut [SidebarItem], top: usize) -> &mut SidebarFolder {
  match &mut items[top] {
    SidebarItem::Folder(folder) => folder,
    SidebarItem::Row(_) => unreachable!("the index names a folder"),
  }
}

fn new_folder(id: FolderId, name: &str, rows: Vec<Entry>) -> SidebarItem {
  SidebarItem::Folder(SidebarFolder {
    id,
    name: name.to_string(),
    collapsed: false,
    rows,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::layout::testing::{core, folder, id, layout_with, top};
  use crate::layout::{Axis, Node};

  fn loose(address: &str) -> Entry {
    Entry::Loose(address.into())
  }

  pub(super) fn row(address: &str) -> SidebarItem {
    SidebarItem::Row(loose(address))
  }

  pub(super) fn in_folder(number: u32, collapsed: bool, rows: &[&str]) -> SidebarItem {
    SidebarItem::Folder(SidebarFolder {
      id: id(number),
      name: format!("f{number}"),
      collapsed,
      rows: rows.iter().map(|address| loose(address)).collect(),
    })
  }

  #[test]
  fn folders_keep_their_place_and_unlisted_rows_go_last() {
    let core = core(&["t1", "t2", "t3", "t4", "t5"]);
    let layout = layout_with(
      &[],
      vec![top("t4"), folder(1, false, &["t3", "t1"]), top("t2")],
    );
    assert_eq!(
      sidebar_items(&layout, &core),
      vec![
        row("t4"),
        in_folder(1, false, &["t3", "t1"]),
        row("t2"),
        row("t5")
      ]
    );
  }

  #[test]
  fn a_tab_sits_in_the_folder_of_its_best_ranked_terminal() {
    let core = core(&["t1", "t2", "t3"]);
    let mut layout = layout_with(
      &["t3"],
      vec![folder(1, false, &["t2"]), top("t3"), top("t1")],
    );
    layout.tabs[0].root = Node::split(Axis::Horizontal, Node::leaf("t3"), Node::leaf("t2"));
    let tab = Entry::Tab {
      index: 0,
      terminals: vec!["t3".into(), "t2".into()],
    };
    let f1 = SidebarFolder {
      id: id(1),
      name: "f1".into(),
      collapsed: false,
      rows: vec![tab],
    };
    assert_eq!(
      sidebar_items(&layout, &core),
      vec![SidebarItem::Folder(f1), row("t1")]
    );
  }

  #[test]
  fn collapsed_and_empty_folders_still_show_with_their_rows() {
    let core = core(&["t1"]);
    let layout = layout_with(&[], vec![folder(1, true, &["t1"]), folder(2, false, &[])]);
    assert_eq!(
      sidebar_items(&layout, &core),
      vec![in_folder(1, true, &["t1"]), in_folder(2, false, &[])]
    );
  }

  #[test]
  fn an_address_listed_twice_keeps_its_first_place() {
    let core = core(&["t1", "t2"]);
    let layout = layout_with(&[], vec![top("t1"), folder(1, false, &["t1", "t2"])]);
    assert_eq!(
      sidebar_items(&layout, &core),
      vec![row("t1"), in_folder(1, false, &["t2"])]
    );
  }

  #[test]
  fn a_folder_of_gone_terminals_shows_empty() {
    let core = core(&["t1"]);
    let layout = layout_with(&[], vec![folder(1, false, &["t9"]), top("t1")]);
    assert_eq!(
      sidebar_items(&layout, &core),
      vec![in_folder(1, false, &[]), row("t1")]
    );
  }

  #[test]
  fn arranging_the_sidebar_as_shown_keeps_its_order() {
    let core = core(&["t1", "t2", "t3"]);
    let mut layout = layout_with(&[], vec![top("t2"), folder(1, true, &["t3"]), top("t1")]);
    let before = layout.slots();
    let items = sidebar_items(&layout, &core);
    layout.arrange(&items);
    assert_eq!(layout.slots(), before);
  }

  #[test]
  fn arranging_lists_unlisted_rows_and_drops_gone_terminals() {
    let core = core(&["t1", "t2"]);
    let mut layout = layout_with(&[], vec![folder(1, false, &["t9", "t2"])]);
    let items = sidebar_items(&layout, &core);
    layout.arrange(&items);
    assert_eq!(layout.slots(), vec![folder(1, false, &["t2"]), top("t1")]);
  }

  #[test]
  fn rows_are_found_inside_folders_too() {
    let items = vec![row("t1"), in_folder(1, false, &["t2"])];
    assert_eq!(find_row(&items, "t2"), Some(&loose("t2")));
    assert_eq!(find_row(&items, "t9"), None);
    assert_eq!(folder_of(&items, "t2"), Some(id(1)));
    assert_eq!(folder_of(&items, "t1"), None);
  }

  #[test]
  fn moving_up_follows_the_folder_rules() {
    let up = |items: Vec<SidebarItem>, moving: &str| moved_row(&items, moving, Direction::Up);
    // In a folder, not first: swaps with the row above.
    assert_eq!(
      up(vec![in_folder(1, false, &["t1", "t2"])], "t2"),
      Some(vec![in_folder(1, false, &["t2", "t1"])])
    );
    // In a folder, first: leaves it and sits right above it.
    assert_eq!(
      up(vec![row("t0"), in_folder(1, false, &["t1", "t2"])], "t1"),
      Some(vec![row("t0"), row("t1"), in_folder(1, false, &["t2"])])
    );
    // Below an open folder, empty or not: goes in last.
    assert_eq!(
      up(vec![in_folder(1, false, &["t1"]), row("t2")], "t2"),
      Some(vec![in_folder(1, false, &["t1", "t2"])])
    );
    assert_eq!(
      up(vec![in_folder(1, false, &[]), row("t2")], "t2"),
      Some(vec![in_folder(1, false, &["t2"])])
    );
    // Below a collapsed folder: jumps above it and stays at the top level.
    assert_eq!(
      up(vec![in_folder(1, true, &["t1"]), row("t2")], "t2"),
      Some(vec![row("t2"), in_folder(1, true, &["t1"])])
    );
    // Below a row: swaps, as today. First at the top level: can't move.
    assert_eq!(
      up(vec![row("t1"), row("t2")], "t2"),
      Some(vec![row("t2"), row("t1")])
    );
    assert_eq!(up(vec![row("t1"), row("t2")], "t1"), None);
  }

  #[test]
  fn moving_down_mirrors_moving_up() {
    let down = |items: Vec<SidebarItem>, moving: &str| moved_row(&items, moving, Direction::Down);
    assert_eq!(
      down(vec![in_folder(1, false, &["t1", "t2"])], "t1"),
      Some(vec![in_folder(1, false, &["t2", "t1"])])
    );
    assert_eq!(
      down(vec![in_folder(1, false, &["t1", "t2"]), row("t3")], "t2"),
      Some(vec![in_folder(1, false, &["t1"]), row("t2"), row("t3")])
    );
    assert_eq!(
      down(vec![row("t1"), in_folder(1, false, &["t2"])], "t1"),
      Some(vec![in_folder(1, false, &["t1", "t2"])])
    );
    assert_eq!(
      down(vec![row("t1"), in_folder(1, false, &[])], "t1"),
      Some(vec![in_folder(1, false, &["t1"])])
    );
    assert_eq!(
      down(vec![row("t1"), in_folder(1, true, &["t2"])], "t1"),
      Some(vec![in_folder(1, true, &["t2"]), row("t1")])
    );
    assert_eq!(
      down(vec![row("t1"), row("t2")], "t1"),
      Some(vec![row("t2"), row("t1")])
    );
    assert_eq!(down(vec![row("t1"), row("t2")], "t2"), None);
  }

  #[test]
  fn moving_within_a_collapsed_folder_opens_it() {
    assert_eq!(
      moved_row(&[in_folder(1, true, &["t1", "t2"])], "t2", Direction::Up),
      Some(vec![in_folder(1, false, &["t2", "t1"])])
    );
  }

  #[test]
  fn a_folder_swaps_with_the_line_next_to_it() {
    let items = vec![
      row("t1"),
      in_folder(1, false, &["t2"]),
      in_folder(2, true, &[]),
    ];
    assert_eq!(
      moved_folder(&items, id(1), Direction::Up),
      Some(vec![
        in_folder(1, false, &["t2"]),
        row("t1"),
        in_folder(2, true, &[])
      ])
    );
    assert_eq!(
      moved_folder(&items, id(1), Direction::Down),
      Some(vec![
        row("t1"),
        in_folder(2, true, &[]),
        in_folder(1, false, &["t2"])
      ])
    );
    assert_eq!(moved_folder(&items, id(2), Direction::Down), None);
    assert_eq!(moved_folder(&items[1..], id(1), Direction::Up), None);
  }

  #[test]
  fn a_new_folder_takes_the_rows_place_or_follows_its_old_folder() {
    assert_eq!(
      moved_to_new_folder(&[row("t1"), row("t2")], "t2", id(3), "f3"),
      Some(vec![row("t1"), in_folder(3, false, &["t2"])])
    );
    assert_eq!(
      moved_to_new_folder(
        &[in_folder(1, false, &["t1", "t2"]), row("t3")],
        "t1",
        id(2),
        "f2"
      ),
      Some(vec![
        in_folder(1, false, &["t2"]),
        in_folder(2, false, &["t1"]),
        row("t3")
      ])
    );
  }

  #[test]
  fn moving_to_a_folder_puts_the_row_last_and_opens_it() {
    let items = vec![row("t1"), in_folder(1, true, &["t2"])];
    assert_eq!(
      moved_to_folder(&items, "t1", id(1)),
      Some(vec![in_folder(1, false, &["t2", "t1"])])
    );
    assert_eq!(moved_to_folder(&items, "t2", id(1)), None);
  }

  #[test]
  fn removing_from_a_folder_puts_the_row_right_below_it() {
    let items = vec![in_folder(1, false, &["t1", "t2"]), row("t3")];
    assert_eq!(
      removed_from_folder(&items, "t1"),
      Some(vec![in_folder(1, false, &["t2"]), row("t1"), row("t3")])
    );
    assert_eq!(removed_from_folder(&items, "t3"), None);
  }

  #[test]
  fn a_new_empty_folder_goes_last_and_open() {
    assert_eq!(
      with_new_folder(&[in_folder(1, true, &[]), row("t1")], id(2), "f2"),
      vec![in_folder(1, true, &[]), row("t1"), in_folder(2, false, &[])]
    );
  }

  #[test]
  fn a_tab_moves_by_any_of_its_terminals() {
    let tab = SidebarItem::Row(Entry::Tab {
      index: 0,
      terminals: vec!["t1".into(), "t2".into()],
    });
    assert_eq!(
      moved_row(&[tab.clone(), row("t3")], "t2", Direction::Down),
      Some(vec![row("t3"), tab])
    );
  }

  #[test]
  fn moving_into_a_missing_folder_does_nothing() {
    assert_eq!(moved_to_folder(&[row("t1")], "t1", id(9)), None);
  }

  #[test]
  fn leaving_a_collapsed_folder_keeps_it_collapsed() {
    assert_eq!(
      moved_row(&[in_folder(1, true, &["t1", "t2"])], "t2", Direction::Down),
      Some(vec![in_folder(1, true, &["t1"]), row("t2")])
    );
  }
}
