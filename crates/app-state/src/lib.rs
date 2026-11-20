//! What the app knows about the core's terminals, kept current from `events.subscribe`.

pub mod layout;
pub mod sidebar;

use hc_proto::types::{Change, Event, StatusState, SubscribeResult, Terminal, TerminalState};

/// The terminals the core reports, kept current by applying its events in order.
#[derive(Debug, Default)]
pub struct Terminals {
  /// The last sequence applied; older events are already in the state.
  seq: u64,
  terminals: Vec<Terminal>,
  /// Closed by the user: on their way out, and removed as soon as they end.
  closing: Vec<String>,
}

impl Terminals {
  /// Starts from the snapshot `events.subscribe` returned.
  pub fn from_snapshot(snapshot: SubscribeResult) -> Self {
    Self {
      seq: snapshot.seq,
      terminals: snapshot.terminals,
      closing: Vec::new(),
    }
  }

  /// After a `missed` notice: the fresh snapshot replaces everything.
  pub fn reset(&mut self, snapshot: SubscribeResult) {
    self.seq = snapshot.seq;
    self.terminals = snapshot.terminals;
    self.closing.clear();
  }

  /// Marks a terminal as closed by the user: the sidebar stops listing it on its own, and
  /// `clean_exits` reports it once it ends.
  pub fn close(&mut self, address: &str) {
    self.closing.push(address.to_string());
  }

  /// Ended cleanly or closed by the user: about to be removed, so no longer listed.
  fn is_leaving(&self, terminal: &Terminal) -> bool {
    let finished = terminal.state == TerminalState::Finished;
    let closed = self.closing.contains(&terminal.address);
    closed || (finished && terminal.exit_code == Some(0))
  }

  /// Every terminal, in the core's order.
  pub fn list(&self) -> &[Terminal] {
    &self.terminals
  }

  /// The terminal at `address`, if the core has it.
  pub fn get(&self, address: &str) -> Option<&Terminal> {
    self
      .terminals
      .iter()
      .find(|terminal| terminal.address == address)
  }

  /// Applies one event; events at or before the last applied sequence are ignored.
  pub fn apply(&mut self, event: &Event) {
    if event.seq <= self.seq {
      return;
    }
    self.seq = event.seq;
    let index = self
      .terminals
      .iter()
      .position(|terminal| terminal.address == event.terminal);
    match (&event.change, index) {
      (Change::Created(terminal), None) => self.terminals.push((**terminal).clone()),
      (Change::Created(terminal), Some(index)) => self.terminals[index] = (**terminal).clone(),
      (Change::Removed, Some(index)) => {
        let removed = self.terminals.remove(index);
        self.closing.retain(|address| *address != removed.address);
      }
      (change, Some(index)) => update(&mut self.terminals[index], change),
      (_, None) => {}
    }
  }
}

fn update(terminal: &mut Terminal, change: &Change) {
  match change {
    Change::Foreground(foreground) => terminal.foreground = Some(foreground.clone()),
    Change::Labels { labels } => terminal.labels = labels.clone(),
    Change::Title { title } => terminal.title = title.clone(),
    Change::Status { status } => terminal.status = status.clone(),
    Change::Finished {
      exit_code, reason, ..
    } => {
      terminal.state = TerminalState::Finished;
      terminal.exit_code = *exit_code;
      terminal.reason = reason.clone();
      terminal.status = None;
      terminal.foreground = None;
    }
    Change::Created(_)
    | Change::Removed
    | Change::Restarted
    | Change::Doc { .. }
    | Change::Other => {}
  }
}

/// Shown terminals that finished: their panes close, as a shell's window closes on `exit`.
/// They stay in the list until removed. One not seen yet was likely just started, so it stays.
pub fn panes_to_close(layout: &layout::Layout, terminals: &Terminals) -> Vec<String> {
  layout
    .shown()
    .into_iter()
    .filter(|address| {
      terminals
        .get(address)
        .is_some_and(|terminal| terminal.state == TerminalState::Finished)
    })
    .map(str::to_string)
    .collect()
}

/// Ended terminals with nothing to look at: exit code 0, like a shell closed with `exit`, or
/// closed by the user. One that failed or was killed on its own stays, to see what happened.
pub fn clean_exits(terminals: &Terminals) -> Vec<String> {
  terminals
    .list()
    .iter()
    .filter(|terminal| terminal.state == TerminalState::Finished && terminals.is_leaving(terminal))
    .map(|terminal| terminal.address.clone())
    .collect()
}

/// A row in the sidebar: a tab with every terminal its panes show, or a terminal shown nowhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
  /// A tab and the terminals its panes show, in reading order.
  Tab {
    index: usize,
    terminals: Vec<String>,
  },
  /// The address of a terminal that no tab shows.
  Loose(String),
}

impl Entry {
  /// The addresses of the terminals this row stands for.
  pub fn terminals(&self) -> Vec<&str> {
    match self {
      Entry::Tab { terminals, .. } => terminals.iter().map(String::as_str).collect(),
      Entry::Loose(address) => vec![address.as_str()],
    }
  }
}

/// What a sidebar row reads: the user's name for it, else its (focused) terminal's.
pub fn entry_label(layout: &layout::Layout, terminals: &Terminals, entry: &Entry) -> String {
  let (custom, address) = match entry {
    Entry::Tab { index, .. } => {
      let tab = &layout.tabs[*index];
      (tab.name.clone(), tab.focused.clone())
    }
    Entry::Loose(address) => (layout.names.get(address).cloned(), address.clone()),
  };
  custom.unwrap_or_else(|| terminals.get(&address).map(display_name).unwrap_or(address))
}

/// The dot next to a terminal in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indicator {
  /// Idle, or a program that never reports status.
  Quiet,
  /// Running something.
  Working,
  /// Blocked on a permission or a question, or in error: a person should look.
  NeedsYou,
  /// Reported done.
  Done,
  /// The terminal's program ended.
  Finished,
}

/// The dot a terminal gets in a list, from its state and reported status.
pub fn indicator(terminal: &Terminal) -> Indicator {
  if terminal.state == TerminalState::Finished {
    return Indicator::Finished;
  }
  match terminal.status.as_ref().map(|status| status.state) {
    Some(StatusState::Working) => Indicator::Working,
    Some(StatusState::Blocked | StatusState::Error) => Indicator::NeedsYou,
    Some(StatusState::Done) => Indicator::Done,
    Some(StatusState::Idle | StatusState::Unknown) | None => Indicator::Quiet,
  }
}

/// The dot for a group of terminals: the one that needs a person most.
pub fn most_urgent(indicators: impl IntoIterator<Item = Indicator>) -> Indicator {
  let urgency = |indicator: &Indicator| match indicator {
    Indicator::NeedsYou => 4,
    Indicator::Working => 3,
    Indicator::Done => 2,
    Indicator::Finished => 1,
    Indicator::Quiet => 0,
  };
  indicators
    .into_iter()
    .max_by_key(urgency)
    .unwrap_or(Indicator::Quiet)
}

/// The dot a sidebar row shows: a loose terminal's own, or the most urgent of a tab's panes.
/// A tab's finished panes don't count: they fold away and close right after, so their color
/// would only flash on the tab.
pub fn row_indicator(entry: &Entry, terminals: &Terminals) -> Indicator {
  match entry {
    Entry::Tab {
      terminals: panes, ..
    } => most_urgent(
      panes
        .iter()
        .filter_map(|address| terminals.get(address))
        .map(indicator)
        .filter(|dot| *dot != Indicator::Finished),
    ),
    Entry::Loose(address) => terminals.get(address).map_or(Indicator::Quiet, indicator),
  }
}

/// What to call a terminal in a list: its name, the title its program set, or what runs in it.
pub fn display_name(terminal: &Terminal) -> String {
  if let Some(name) = &terminal.name {
    return name.clone();
  }
  if !terminal.title.is_empty() {
    return terminal.title.clone();
  }
  match &terminal.foreground {
    Some(foreground) => foreground.name.clone(),
    None => terminal
      .argv
      .first()
      .cloned()
      .unwrap_or_else(|| terminal.address.clone()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_most_urgent_indicator_wins() {
    let urgent = |dots: &[Indicator]| most_urgent(dots.iter().copied());
    assert_eq!(
      urgent(&[Indicator::Quiet, Indicator::Done, Indicator::Working]),
      Indicator::Working
    );
    assert_eq!(
      urgent(&[Indicator::Finished, Indicator::NeedsYou]),
      Indicator::NeedsYou
    );
    assert_eq!(
      urgent(&[Indicator::Finished, Indicator::Done]),
      Indicator::Done
    );
    assert_eq!(
      urgent(&[Indicator::Quiet, Indicator::Finished]),
      Indicator::Finished
    );
    assert_eq!(urgent(&[]), Indicator::Quiet);
  }

  #[test]
  fn a_tab_dot_skips_finished_panes_and_a_loose_one_shows_its_own() {
    use hc_proto::types::{Status, StatusState, TerminalState};
    let mut terminals = crate::layout::testing::core(&["t1", "t2", "t3"]);
    terminals.terminals[1].state = TerminalState::Finished;
    terminals.terminals[2].state = TerminalState::Finished;
    let tab = |panes: &[&str]| Entry::Tab {
      index: 0,
      terminals: panes.iter().map(|pane| pane.to_string()).collect(),
    };
    assert_eq!(
      row_indicator(&tab(&["t1", "t2"]), &terminals),
      Indicator::Quiet
    );
    terminals.terminals[0].status = Some(Status {
      state: StatusState::Working,
      title: String::new(),
      msg: String::new(),
      progress: None,
    });
    assert_eq!(
      row_indicator(&tab(&["t1", "t2"]), &terminals),
      Indicator::Working
    );
    assert_eq!(
      row_indicator(&Entry::Loose("t3".into()), &terminals),
      Indicator::Finished
    );
  }
}
