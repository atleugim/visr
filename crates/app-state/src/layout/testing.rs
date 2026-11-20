//! Builders the tests of this crate share.

use super::*;

pub(crate) fn id(number: u32) -> FolderId {
  FolderId(number)
}

pub(crate) fn top(address: &str) -> Slot {
  Slot::Terminal(address.into())
}

/// A folder named `f{number}`.
pub(crate) fn folder(number: u32, collapsed: bool, terminals: &[&str]) -> Slot {
  Slot::Folder(Folder {
    id: id(number),
    name: format!("f{number}"),
    collapsed,
    terminals: terminals
      .iter()
      .map(|address| address.to_string())
      .collect(),
  })
}

/// Live terminals in this order, as the core would report them.
pub(crate) fn core(addresses: &[&str]) -> crate::Terminals {
  use hc_proto::types::{SubscribeResult, Terminal, TerminalState};
  let terminals = addresses
    .iter()
    .map(|address| Terminal {
      address: address.to_string(),
      name: None,
      argv: Vec::new(),
      cwd: String::new(),
      state: TerminalState::Live,
      labels: Default::default(),
      cols: 80,
      rows: 24,
      title: String::new(),
      status: None,
      foreground: None,
      template: None,
      exit_code: None,
      reason: None,
    })
    .collect();
  crate::Terminals::from_snapshot(SubscribeResult {
    seq: 0,
    channel: None,
    terminals,
  })
}

/// One single-pane tab per address, so the last one is active.
pub(crate) fn layout_with(tabs: &[&str], order: Vec<Slot>) -> Layout {
  let mut layout = Layout::default();
  for address in tabs {
    layout.new_tab(address);
  }
  layout.order = order;
  layout
}
