//! Tabs of panes: each tab is a binary tree of splits whose leaves show one terminal each.
//! Closing a pane only stops showing its terminal; the terminal lives on in the core.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Entry;
use crate::sidebar::SidebarItem;

/// The smallest share of the space a split gives its first child.
const MIN_RATIO: f32 = 0.1;
/// The largest share of the space a split gives its first child.
const MAX_RATIO: f32 = 0.9;

/// The direction a split lays out its children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
  /// Side by side.
  Horizontal,
  /// One above the other.
  Vertical,
}

/// One step down a split: which child to take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Branch {
  /// The first child of the split.
  First,
  /// The second child of the split.
  Second,
}

/// A pane layout: a leaf shows one terminal, a split divides its space between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
  /// A pane showing the terminal at this address.
  Leaf { terminal: String },
  /// Two nodes side by side or one above the other.
  Split {
    axis: Axis,
    /// The first child's share of the space.
    ratio: f32,
    first: Box<Node>,
    second: Box<Node>,
  },
}

impl Node {
  /// A pane that shows `terminal`.
  pub fn leaf(terminal: &str) -> Self {
    Self::Leaf {
      terminal: terminal.into(),
    }
  }

  /// An even split of `first` and `second` along `axis`.
  pub fn split(axis: Axis, first: Node, second: Node) -> Self {
    Self::Split {
      axis,
      ratio: 0.5,
      first: Box::new(first),
      second: Box::new(second),
    }
  }

  /// Terminals in reading order: left to right, top to bottom.
  pub fn leaves(&self) -> Vec<&str> {
    match self {
      Self::Leaf { terminal } => vec![terminal.as_str()],
      Self::Split { first, second, .. } => {
        let mut leaves = first.leaves();
        leaves.extend(second.leaves());
        leaves
      }
    }
  }

  /// The ratio of the split at `path`, if a split is there.
  pub fn ratio_at(&self, path: &[Branch]) -> Option<f32> {
    match (self, path.split_first()) {
      (Self::Split { ratio, .. }, None) => Some(*ratio),
      (Self::Split { first, .. }, Some((Branch::First, rest))) => first.ratio_at(rest),
      (Self::Split { second, .. }, Some((Branch::Second, rest))) => second.ratio_at(rest),
      (Self::Leaf { .. }, _) => None,
    }
  }

  /// The pixels the terminal's pane takes once nothing is moving, from the splits' ratios and
  /// the room `size` this node has; `divider` is what each split keeps for its handle.
  pub fn extent_of(&self, terminal: &str, size: (f32, f32), divider: f32) -> Option<(f32, f32)> {
    match self {
      Self::Leaf { terminal: shown } => (shown == terminal).then_some(size),
      Self::Split {
        axis,
        ratio,
        first,
        second,
      } => {
        let (width, height) = size;
        let (first_size, second_size) = match axis {
          Axis::Horizontal => {
            let first = width * ratio;
            (
              (first, height),
              ((width - first - divider).max(0.0), height),
            )
          }
          Axis::Vertical => {
            let first = height * ratio;
            ((width, first), (width, (height - first - divider).max(0.0)))
          }
        };
        first
          .extent_of(terminal, first_size, divider)
          .or_else(|| second.extent_of(terminal, second_size, divider))
      }
    }
  }

  /// The branches from here down to the terminal's leaf.
  pub fn path_to(&self, terminal: &str) -> Option<Vec<Branch>> {
    match self {
      Self::Leaf { terminal: shown } => (shown == terminal).then(Vec::new),
      Self::Split { first, second, .. } => {
        let (branch, path) = match first.path_to(terminal) {
          Some(path) => (Branch::First, path),
          None => (Branch::Second, second.path_to(terminal)?),
        };
        Some([vec![branch], path].concat())
      }
    }
  }

  fn contains(&self, terminal: &str) -> bool {
    self.leaves().contains(&terminal)
  }

  fn leaf_mut(&mut self, terminal: &str) -> Option<&mut Node> {
    match self {
      Self::Leaf { terminal: shown } if shown == terminal => Some(self),
      Self::Leaf { .. } => None,
      Self::Split { first, second, .. } => match first.leaf_mut(terminal) {
        Some(leaf) => Some(leaf),
        None => second.leaf_mut(terminal),
      },
    }
  }

  /// The tree without that terminal's leaf; `None` when nothing is left.
  fn without(self, terminal: &str) -> Option<Node> {
    match self {
      Self::Leaf { terminal: shown } if shown == terminal => None,
      leaf @ Self::Leaf { .. } => Some(leaf),
      Self::Split {
        axis,
        ratio,
        first,
        second,
      } => match (first.without(terminal), second.without(terminal)) {
        (Some(first), Some(second)) => Some(Self::Split {
          axis,
          ratio,
          first: Box::new(first),
          second: Box::new(second),
        }),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
      },
    }
  }

  fn at_mut(&mut self, path: &[Branch]) -> Option<&mut Node> {
    let Some((step, rest)) = path.split_first() else {
      return Some(self);
    };
    match (self, step) {
      (Self::Split { first, .. }, Branch::First) => first.at_mut(rest),
      (Self::Split { second, .. }, Branch::Second) => second.at_mut(rest),
      (Self::Leaf { .. }, _) => None,
    }
  }
}

/// A size the new pane of a split will hold, from the size of the pane it splits: half, less what
/// the divider and the new header take, rounded down. A program never hears it is bigger than it
/// is; the exact size follows once the pane is laid out.
pub fn split_size((cols, rows): (u16, u16), axis: Axis) -> (u16, u16) {
  let half = |cells: u16, chrome: u16| (cells.saturating_sub(chrome) / 2).max(1);
  match axis {
    Axis::Horizontal => (half(cols, 4), rows),
    Axis::Vertical => (cols, half(rows, 6)),
  }
}

/// A tab: its tree of panes and which pane has focus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tab {
  pub root: Node,
  pub focused: String,
  /// The user's name for the tab, in place of its terminal's.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

impl Tab {
  fn new(terminal: &str) -> Self {
    Self {
      root: Node::leaf(terminal),
      focused: terminal.into(),
      name: None,
    }
  }

  /// Drops the terminal's pane; `false` when the tab has nothing left.
  fn forget(&mut self, terminal: &str) -> bool {
    let index = self
      .root
      .leaves()
      .iter()
      .position(|shown| *shown == terminal);
    let root = std::mem::replace(&mut self.root, Node::leaf(""));
    let Some(root) = root.without(terminal) else {
      return false;
    };
    self.root = root;
    if self.focused == terminal {
      let leaves = self.root.leaves();
      let previous = index.unwrap_or(0).saturating_sub(1).min(leaves.len() - 1);
      self.focused = leaves[previous].to_string();
    }
    true
  }
}

/// Names a folder in the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FolderId(u32);

/// The bare number, for element ids.
impl fmt::Display for FolderId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// A named group of sidebar rows, kept in Visr's layout only.
///
/// Its terminals also appear in `Layout::order`, starting at `position`, so a build that doesn't
/// know folders still reads a sensible flat order and ignores the folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
  pub id: FolderId,
  pub name: String,
  #[serde(default)]
  pub collapsed: bool,
  /// Where in `Layout::order` its terminals start, which is also where an empty folder sits.
  /// Rewritten on every edit, so callers don't set it.
  #[serde(default)]
  pub position: usize,
  /// The terminals of its rows, in the order shown.
  #[serde(default)]
  pub terminals: Vec<String>,
}

/// One place in the sidebar's arranged order, with a folder's terminals nested in it. This is the
/// shape the layout is edited in; `Layout::set_slots` flattens it into `order` and `folders`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Slot {
  Terminal(String),
  Folder {
    id: FolderId,
    name: String,
    collapsed: bool,
    terminals: Vec<String>,
  },
}

/// One line the layout lists, as `Layout::places` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place<'a> {
  Folder(&'a Folder),
  Terminal {
    address: &'a str,
    folder: Option<FolderId>,
  },
}

/// The tabs, which one is active, and the user's ordering and names for sidebar rows.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Layout {
  pub tabs: Vec<Tab>,
  pub active_tab: usize,
  /// The sidebar's order as the user arranged it, folder terminals included at their folder's
  /// position; terminals missing from it go last. Builds from before folders read only this.
  #[serde(default)]
  pub order: Vec<String>,
  /// The sidebar's folders. Builds from before folders ignore this and a layout saved by them has
  /// none.
  #[serde(default)]
  pub folders: Vec<Folder>,
  /// The user's names for terminals shown in no tab, by address.
  #[serde(default)]
  pub names: BTreeMap<String, String>,
}

impl Layout {
  /// Every terminal some tab shows, tab by tab.
  pub fn shown(&self) -> Vec<&str> {
    self.tabs.iter().flat_map(|tab| tab.root.leaves()).collect()
  }

  /// The active tab, if any.
  pub fn active(&self) -> Option<&Tab> {
    self.tabs.get(self.active_tab)
  }

  fn active_mut(&mut self) -> Option<&mut Tab> {
    self.tabs.get_mut(self.active_tab)
  }

  /// The terminal in the focused pane of the active tab.
  pub fn focused(&self) -> Option<&str> {
    self.active().map(|tab| tab.focused.as_str())
  }

  /// Opens a tab showing the terminal and makes it active.
  pub fn new_tab(&mut self, terminal: &str) {
    self.tabs.push(Tab::new(terminal));
    self.active_tab = self.tabs.len() - 1;
  }

  /// Makes the tab at `index` active; out of range does nothing.
  pub fn select_tab(&mut self, index: usize) {
    if index < self.tabs.len() {
      self.active_tab = index;
    }
  }

  /// Focuses the terminal's pane if the active tab shows it.
  pub fn focus(&mut self, terminal: &str) {
    if let Some(tab) = self.active_mut()
      && tab.root.contains(terminal)
    {
      tab.focused = terminal.into();
    }
  }

  /// Moves focus to the next pane of the active tab, wrapping around.
  pub fn focus_next(&mut self) {
    self.step_focus(1);
  }

  /// Moves focus to the previous pane of the active tab, wrapping around.
  pub fn focus_prev(&mut self) {
    self.step_focus(-1);
  }

  fn step_focus(&mut self, step: isize) {
    let Some(tab) = self.active_mut() else { return };
    let leaves = tab.root.leaves();
    let Some(index) = leaves.iter().position(|shown| *shown == tab.focused) else {
      return;
    };
    let next = (index as isize + step).rem_euclid(leaves.len() as isize) as usize;
    tab.focused = leaves[next].to_string();
  }

  /// Shows the terminal next to the focused pane, in a new tab if there is none.
  pub fn split(&mut self, axis: Axis, terminal: &str) {
    let Some(tab) = self.active_mut() else {
      self.new_tab(terminal);
      return;
    };
    let focused = tab.focused.clone();
    let shown: Vec<String> = tab.root.leaves().into_iter().map(str::to_string).collect();
    if let Some(leaf) = tab.root.leaf_mut(&focused) {
      *leaf = Node::split(axis, Node::leaf(&focused), Node::leaf(terminal));
      tab.focused = terminal.into();
    }
    self.list_after(&shown, terminal);
  }

  /// Lists `terminal` right after the last of `neighbors` that `order` lists, in its folder or at
  /// the top level. Lists nothing when none is listed. An earlier listing of `terminal` goes first
  /// so an address appears once.
  fn list_after(&mut self, neighbors: &[String], terminal: &str) {
    self.unlist(terminal);
    let last = self
      .places()
      .into_iter()
      .rev()
      .find_map(|place| match place {
        Place::Terminal { address, folder } if neighbors.iter().any(|n| n == address) => {
          Some((address.to_string(), folder))
        }
        _ => None,
      });
    let Some((after, folder)) = last else {
      return;
    };
    let mut slots = self.slots();
    match folder {
      None => {
        let slot = |known: &Slot| matches!(known, Slot::Terminal(a) if *a == after);
        if let Some(index) = slots.iter().position(slot) {
          slots.insert(index + 1, Slot::Terminal(terminal.into()));
        }
      }
      Some(id) => {
        for known in &mut slots {
          if let Slot::Folder {
            id: known_id,
            terminals,
            ..
          } = known
            && *known_id == id
            && let Some(index) = terminals.iter().position(|a| *a == after)
          {
            terminals.insert(index + 1, terminal.into());
          }
        }
      }
    }
    self.set_slots(slots);
  }

  /// Drops every listing of `terminal`; folders stay even when empty.
  fn unlist(&mut self, terminal: &str) {
    let mut slots = self.slots();
    slots.retain(|slot| !matches!(slot, Slot::Terminal(known) if known == terminal));
    for slot in &mut slots {
      if let Slot::Folder { terminals, .. } = slot {
        terminals.retain(|known| known != terminal);
      }
    }
    self.set_slots(slots);
  }

  /// The sidebar order as nested slots, healed the way `places` reads it.
  pub(crate) fn slots(&self) -> Vec<Slot> {
    let mut slots = Vec::new();
    for place in self.places() {
      match (place, slots.last_mut()) {
        (Place::Folder(folder), _) => slots.push(Slot::Folder {
          id: folder.id,
          name: folder.name.clone(),
          collapsed: folder.collapsed,
          terminals: Vec::new(),
        }),
        (
          Place::Terminal {
            address,
            folder: Some(_),
          },
          Some(Slot::Folder { terminals, .. }),
        ) => terminals.push(address.to_string()),
        (Place::Terminal { address, .. }, _) => slots.push(Slot::Terminal(address.to_string())),
      }
    }
    slots
  }

  /// Writes slots to `order` and `folders`, so both views agree.
  pub(crate) fn set_slots(&mut self, slots: Vec<Slot>) {
    self.order.clear();
    self.folders.clear();
    for slot in slots {
      match slot {
        Slot::Terminal(address) => self.order.push(address),
        Slot::Folder {
          id,
          name,
          collapsed,
          terminals,
        } => {
          let position = self.order.len();
          self.order.extend(terminals.iter().cloned());
          self.folders.push(Folder {
            id,
            name,
            collapsed,
            position,
            terminals,
          });
        }
      }
    }
  }

  /// The folder with this id.
  pub fn folder(&self, id: FolderId) -> Option<&Folder> {
    self.folders.iter().find(|folder| folder.id == id)
  }

  fn folder_mut(&mut self, id: FolderId) -> Option<&mut Folder> {
    self.folders.iter_mut().find(|folder| folder.id == id)
  }

  /// One more than the largest folder id, or 1 when there is none.
  pub fn next_folder_id(&self) -> FolderId {
    let largest = self.folders.iter().map(|folder| folder.id.0).max();
    FolderId(largest.map_or(1, |largest| largest + 1))
  }

  /// Trimmed; a blank name keeps the current one, because a folder always has a name.
  pub fn rename_folder(&mut self, id: FolderId, name: &str) {
    let name = name.trim();
    if let Some(folder) = self.folder_mut(id).filter(|_| !name.is_empty()) {
      folder.name = name.to_string();
    }
  }

  /// Collapses an open folder and opens a collapsed one.
  pub fn toggle_folder(&mut self, id: FolderId) {
    if let Some(folder) = self.folder_mut(id) {
      folder.collapsed = !folder.collapsed;
    }
  }

  /// Its terminals take its place at the top level, in their order. No terminal is affected.
  pub fn delete_folder(&mut self, id: FolderId) {
    let slots = self
      .slots()
      .into_iter()
      .flat_map(|slot| match slot {
        Slot::Folder {
          id: known,
          terminals,
          ..
        } if known == id => terminals.into_iter().map(Slot::Terminal).collect(),
        other => vec![other],
      })
      .collect();
    self.set_slots(slots);
  }

  /// Writes the sidebar as shown back to `order` and `folders`: one place per terminal, folders
  /// with their rows. Terminals the sidebar doesn't show drop out.
  pub fn arrange(&mut self, items: &[SidebarItem]) {
    let addresses = |entry: &Entry| {
      entry
        .terminals()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
    };
    let slots = items
      .iter()
      .flat_map(|item| match item {
        SidebarItem::Row(entry) => addresses(entry).into_iter().map(Slot::Terminal).collect(),
        SidebarItem::Folder(folder) => vec![Slot::Folder {
          id: folder.id,
          name: folder.name.clone(),
          collapsed: folder.collapsed,
          terminals: folder.rows.iter().flat_map(addresses).collect(),
        }],
      })
      .collect();
    self.set_slots(slots);
  }

  /// Stops showing the focused pane; a tab left empty closes.
  pub fn close_focused(&mut self) {
    let Some(focused) = self.focused().map(str::to_string) else {
      return;
    };
    let active = self.active_tab;
    if !self.tabs[active].forget(&focused) {
      self.tabs.remove(active);
      self.active_tab = active.min(self.tabs.len().saturating_sub(1));
    }
  }

  /// Closes the active tab and all its panes; their terminals live on.
  pub fn close_tab(&mut self) {
    if self.active_tab < self.tabs.len() {
      self.tabs.remove(self.active_tab);
      self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
    }
  }

  /// Every place the layout lists, depth first: a folder, then its terminals. A folder comes
  /// before the `order` entry at its `position`, and `order` entries in the range its terminals
  /// cover belong to it. A terminal listed again later keeps its first place, and a repeated
  /// folder id is skipped.
  pub(crate) fn places(&self) -> Vec<Place<'_>> {
    fn push_folder<'a>(places: &mut Vec<Place<'a>>, folder: &'a Folder) {
      places.push(Place::Folder(folder));
      places.extend(folder.terminals.iter().map(|address| Place::Terminal {
        address,
        folder: Some(folder.id),
      }));
    }

    let mut folders: Vec<&Folder> = Vec::with_capacity(self.folders.len());
    for folder in &self.folders {
      if !folders.iter().any(|known| known.id == folder.id) {
        folders.push(folder);
      }
    }
    folders.sort_by_key(|folder| folder.position);
    let in_folder = |index: usize, address: &str| {
      folders.iter().any(|folder| {
        (folder.position..folder.position + folder.terminals.len()).contains(&index)
          && folder.terminals.iter().any(|known| known == address)
      })
    };
    let mut places = Vec::new();
    let mut pending = folders.iter().copied().peekable();
    for (index, address) in self.order.iter().enumerate() {
      while let Some(folder) = pending.next_if(|folder| folder.position <= index) {
        push_folder(&mut places, folder);
      }
      if !in_folder(index, address) {
        places.push(Place::Terminal {
          address,
          folder: None,
        });
      }
    }
    for folder in pending {
      push_folder(&mut places, folder);
    }
    let mut seen: Vec<&str> = Vec::new();
    places.retain(|place| match place {
      Place::Terminal { address, .. } => {
        let first = !seen.contains(address);
        seen.push(address);
        first
      }
      Place::Folder(_) => true,
    });
    places
  }

  /// Names a sidebar row for this view only; the terminals keep theirs. Blank clears it.
  pub fn rename(&mut self, entry: &crate::Entry, name: &str) {
    let name = Some(name.trim().to_string()).filter(|name| !name.is_empty());
    match (entry, name) {
      (crate::Entry::Tab { index, .. }, name) => {
        if let Some(tab) = self.tabs.get_mut(*index) {
          tab.name = name;
        }
      }
      (crate::Entry::Loose(address), Some(name)) => {
        self.names.insert(address.clone(), name);
      }
      (crate::Entry::Loose(address), None) => {
        self.names.remove(address);
      }
    }
  }

  /// The terminal is gone from the core: drop its place and name. Its panes are left to
  /// `stop_showing`.
  pub fn forget(&mut self, terminal: &str) {
    self.unlist(terminal);
    self.names.remove(terminal);
  }

  /// Drops the terminal's panes everywhere; its place and name stay.
  pub fn stop_showing(&mut self, terminal: &str) {
    self.tabs.retain_mut(|tab| tab.forget(terminal));
    self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
  }

  /// Stops showing every shown terminal not in `terminals`; their places and names stay.
  pub fn keep_only(&mut self, terminals: &[&str]) {
    let shown: Vec<String> = self.shown().into_iter().map(str::to_string).collect();
    for terminal in shown
      .iter()
      .filter(|shown| !terminals.contains(&shown.as_str()))
    {
      self.stop_showing(terminal);
    }
  }

  /// Brings a terminal into view: its tab if it has one, else in place of the focused pane when
  /// that tab is in the terminal's place, else in a tab of its own. A tab never mixes two
  /// folders, so a terminal from another place gets its own tab, which ranks where its row was.
  pub fn show(&mut self, terminal: &str) {
    if let Some(index) = self.tabs.iter().position(|tab| tab.root.contains(terminal)) {
      self.active_tab = index;
      self.tabs[index].focused = terminal.into();
      return;
    }
    let same_place = self
      .active()
      .is_some_and(|tab| self.tab_folder(tab) == self.terminal_folder(terminal));
    if !same_place {
      self.new_tab(terminal);
      return;
    }
    let Some(tab) = self.active_mut() else { return };
    let focused = tab.focused.clone();
    if let Some(leaf) = tab.root.leaf_mut(&focused) {
      *leaf = Node::leaf(terminal);
      tab.focused = terminal.into();
    }
  }

  /// Opens a tab like `new_tab`; when the tab that was active sits in a folder, the terminal goes
  /// last in that folder and the folder opens. Otherwise it stays unlisted, at the top level.
  pub fn new_tab_in_active_folder(&mut self, terminal: &str) {
    let folder = self.active().and_then(|tab| self.tab_folder(tab));
    self.new_tab(terminal);
    let Some(id) = folder else { return };
    self.unlist(terminal);
    let mut slots = self.slots();
    for slot in &mut slots {
      if let Slot::Folder {
        id: known,
        terminals,
        collapsed,
        ..
      } = slot
        && *known == id
      {
        terminals.push(terminal.into());
        *collapsed = false;
      }
    }
    self.set_slots(slots);
  }

  /// The folder a terminal is listed in; `None` at the top level or when unlisted.
  fn terminal_folder(&self, terminal: &str) -> Option<FolderId> {
    self.places().into_iter().find_map(|place| match place {
      Place::Terminal { address, folder } if address == terminal => Some(folder),
      _ => None,
    })?
  }

  /// The folder of a tab: that of its first terminal in `places()`, `None` when none is listed.
  fn tab_folder(&self, tab: &Tab) -> Option<FolderId> {
    let shown = tab.root.leaves();
    self.places().into_iter().find_map(|place| match place {
      Place::Terminal { address, folder } if shown.contains(&address) => Some(folder),
      _ => None,
    })?
  }

  /// Sets the split at `path` in the active tab, kept between `MIN_RATIO` and `MAX_RATIO`.
  pub fn set_ratio(&mut self, path: &[Branch], ratio: f32) {
    if let Some(tab) = self.active_mut()
      && let Some(Node::Split { ratio: current, .. }) = tab.root.at_mut(path)
    {
      *current = ratio.clamp(MIN_RATIO, MAX_RATIO);
    }
  }
}

#[cfg(test)]
pub(crate) mod testing;

#[cfg(test)]
mod tests;
