//! Talking to the core: the connection's notices, saving the layout, and starting and ending
//! terminals.

use super::*;

impl Workspace {
  /// A new connection after the old one dropped: fresh state, and every pane attaches again.
  pub(super) fn reconnected(
    &mut self,
    client: Arc<Client>,
    snapshot: SubscribeResult,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    crate::about::set_core_version(&client.hello().version, cx);
    self.client = client;
    self.terminals.reset(snapshot);
    self.reconnecting = false;
    self.panes.clear();
    for address in panes_to_close(&self.layout, &self.terminals) {
      self.layout.stop_showing(&address);
    }
    self.remove_clean_exits();
    self.changed(window, cx);
  }

  pub(super) fn remove_clean_exits(&self) {
    for terminal in clean_exits(&self.terminals) {
      let _ = self
        .client
        .call::<_, RemoveResult>("terminal.rm", &TerminalParams { terminal });
    }
  }

  pub(super) fn save(&mut self) {
    let saved = self
      .client
      .doc_set(LAYOUT_DOC, &self.layout, self.layout_version);
    // Someone else wrote it since: this window's layout wins. Any other failure is not a reason
    // to write again.
    let saved = match saved {
      Err(error) if error.is_conflict() => self
        .client
        .doc_get::<serde_json::Value>(LAYOUT_DOC)
        .and_then(|(_, version)| self.client.doc_set(LAYOUT_DOC, &self.layout, version)),
      other => other,
    };
    if let Ok(version) = saved {
      self.layout_version = version;
    }
  }

  pub(super) fn sync_panes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    // Every tab keeps its views, not just the one in sight: coming back to a tab shows its
    // screen as it is, instead of attaching again and redrawing at a new size.
    let shown: Vec<String> = self
      .layout
      .shown()
      .into_iter()
      .map(str::to_string)
      .collect();
    self.panes.retain(|address, _| shown.contains(address));
    for address in &shown {
      if self.panes.contains_key(address) {
        continue;
      }
      match self.open_pane(address.clone(), window, cx) {
        Some(pane) => {
          self.panes.insert(address.clone(), pane);
        }
        // Gone or finished since the layout was saved: drop its pane.
        None => self.layout.stop_showing(address),
      }
    }
    // A row or folder being renamed keeps the keyboard until it is done.
    if self.renaming.is_none()
      && let Some(focused) = self.layout.focused()
      && let Some(pane) = self.panes.get(focused)
    {
      pane.view.read(cx).focus(window);
    }
  }

  fn open_pane(
    &mut self,
    address: String,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<Pane> {
    let size = self.started_sizes.remove(&address);
    let attachment =
      TerminalView::attach(&self.client, address.clone(), size, &self.config, window).ok()?;
    let config = &self.config;
    let view = cx.new(|cx| TerminalView::new(attachment, config, window, cx));
    let printed = address.clone();
    let output = cx.subscribe(&view, move |workspace, _, _: &Output, cx| {
      workspace.note_output(&printed, cx);
    });
    let focused = cx.subscribe(&view, move |workspace, _, _: &PaneFocused, cx| {
      workspace.layout.focus(&address);
      workspace.save();
      workspace.note_focus(cx);
      cx.notify();
    });
    Some(Pane {
      view,
      _focused: focused,
      _output: output,
    })
  }

  pub(super) fn on_notice(&mut self, notice: Notice, window: &mut Window, cx: &mut Context<Self>) {
    let before = self.layout.clone();
    let mut reattach = false;
    let mut finished = false;
    match notice {
      Notice::Event(event) => {
        match event.change {
          // Its stream ended with the lost program; dropping the pane makes it attach again.
          Change::Restarted => reattach = self.panes.remove(&event.terminal).is_some(),
          Change::Removed => {
            self.layout.forget(&event.terminal);
            // A pane folding away leaves the layout when its motion ends.
            if !self.is_closing(&event.terminal) {
              self.layout.stop_showing(&event.terminal);
            }
          }
          Change::Finished { .. } => finished = true,
          _ => {}
        }
        self.terminals.apply(&event);
        // What the panel shows follows the active terminal's folder, which changes with its
        // foreground program and status.
        let of_panel = matches!(
          event.change,
          Change::Foreground(_) | Change::Status { .. } | Change::Finished { .. }
        );
        if of_panel && self.layout.focused() == Some(event.terminal.as_str()) {
          self.refresh_panel(cx);
        }
        if finished {
          self.remove_clean_exits();
          // An editor run in a pane may have just saved `config.toml`.
          self.check_config(window, cx);
        }
      }
      Notice::Missed(params) => {
        if let Some(snapshot) = missed_snapshot(params) {
          self.terminals.reset(snapshot);
        }
      }
      Notice::Heartbeat | Notice::Other { .. } => {}
    }
    for address in panes_to_close(&self.layout, &self.terminals) {
      if !self.is_closing(&address) {
        self.close_pane(&address, window, cx);
      }
    }
    // Saving only on a real change: a save is itself a `doc` event, so saving on every notice
    // would never stop.
    if self.layout != before {
      self.changed(window, cx);
    } else if reattach {
      self.sync_panes(window, cx);
      cx.notify();
    } else {
      cx.notify();
    }
  }

  /// An empty argv starts the machine's own shell; a template name starts that template.
  /// `size` is what the program should first think its pane is: no bigger than the pane will be,
  /// or its first lines wrap (zsh shows a stray `%`).
  pub(super) fn start(
    &mut self,
    argv: Vec<String>,
    size: Option<(u16, u16)>,
    cwd: Option<String>,
  ) -> Option<String> {
    let cwd = cwd.or_else(|| std::env::home_dir().map(|home| home.to_string_lossy().into_owned()));
    let params = RunParams {
      argv,
      cwd,
      size: size.map(|(cols, rows)| Size { cols, rows }),
      ..Default::default()
    };
    let terminal = self
      .client
      .call::<_, Terminal>("terminal.run", &params)
      .ok()?;
    // Its first attach keeps this size, so its shell is not resized right after it starts.
    self
      .started_sizes
      .insert(terminal.address.clone(), (terminal.cols, terminal.rows));
    Some(terminal.address)
  }

  /// Kills a live terminal; removes a finished one.
  pub(super) fn close_address(&mut self, address: &str) {
    if let Some(terminal) = self.terminals.get(address).cloned() {
      self.end(&terminal);
    }
  }

  /// Kills a live terminal, to be removed once it ends; removes a finished one.
  fn end(&mut self, terminal: &Terminal) {
    self.terminals.close(&terminal.address);
    let params = TerminalParams {
      terminal: terminal.address.clone(),
    };
    if terminal.state == TerminalState::Live {
      let _ = self.client.call::<_, Terminal>("terminal.kill", &params);
    } else {
      let _ = self.client.call::<_, RemoveResult>("terminal.rm", &params);
    }
  }
}
