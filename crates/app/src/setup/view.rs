//! The window shown when herdr-core is missing or out of date: what will be installed and from
//! where, the install as it runs, and the way into the app once it is there.

use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use config::Config;
use gpui::{
  AnyElement, ClipboardItem, Context, FocusHandle, KeyDownEvent, SharedString, Stateful, Window,
  div, prelude::*, px,
};
use hc_client::Error;
use hc_proto::PROTOCOL_GENERATION;
use hc_proto::rpc::ErrorKind;
use ui::colors::Colors;
use ui::link::Link;
use ui::motion::{self, Tween};

use super::install::{self, Cancel, Event};
use super::manifest::{self, MANUAL_URL, Manifest, Plan, Refusal};
use crate::connection::Connection;
use crate::startup::startup_message;
use crate::workspace::Workspace;

/// How much of the installer's output the window shows.
const LOG_LINES: usize = 14;
/// What is kept of the installer's output, for Copy log.
const LOG_KEPT: usize = 5000;
const CONNECT_TRIES: usize = 3;

/// Why the window is here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reason {
  /// There is no herdr-core to run.
  Missing,
  /// The one there speaks another protocol generation.
  OutOfDate,
}

/// Whether a failure to start is one that installing herdr-core could fix.
pub(crate) fn reason(error: &Error) -> Option<Reason> {
  match error {
    Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => Some(Reason::Missing),
    Error::Rpc(rpc) if rpc.kind == Some(ErrorKind::UnsupportedGeneration) => {
      Some(Reason::OutOfDate)
    }
    _ => None,
  }
}

/// What the workspace needs once it opens, kept until then.
pub(crate) struct Launch {
  pub(crate) initial: Option<String>,
  pub(crate) config: Config,
  pub(crate) config_error: Option<String>,
  pub(crate) theme_notice: Option<SharedString>,
}

/// What the retry button does after a failure.
#[derive(Debug, Clone)]
enum Retry {
  Check,
  Install(Plan),
  Connect,
}

/// Which step the setup window is on.
enum State {
  Checking,
  Ready(Plan),
  Installing(Plan),
  Connecting,
  /// herdr-core answers. The window waits here for the person to open the app.
  Done,
  Failed {
    message: String,
    retry: Retry,
  },
  Unsupported(String),
}

/// The setup window's view: checks herdr-core, offers to install it and reports progress.
pub(crate) struct Setup {
  reason: Reason,
  state: State,
  /// What the installer said, as it said it.
  log: Vec<String>,
  cancel: Option<Cancel>,
  /// Where the installer was saved, removed when it is over.
  scratch: Option<PathBuf>,
  launch: Option<Launch>,
  /// The version the installer finished installing, for the message at the end: only an install
  /// that ended well sets it, so a plain reconnect names no version.
  installed: Option<String>,
  /// The connection that showed it works, kept until the person opens the app. The workspace
  /// copes with one that has gone stale or dropped meanwhile: it replays what it queued, asks for
  /// a fresh snapshot after a `missed`, and reconnects.
  connection: Option<Connection>,
  focus_handle: FocusHandle,
  /// The screen now shown coming in, and the kind of screen it is.
  entrance: Tween,
  shown: Option<std::mem::Discriminant<State>>,
  /// The setup going out, before the app takes the window.
  leaving: Option<Tween>,
  _task: Option<gpui::Task<()>>,
}

fn no_build_message(os: &str, arch: &str) -> String {
  if os == "windows" {
    "herdr-core has no build for Windows on ARM yet.".to_string()
  } else {
    format!("herdr-core has no build for {os} on {arch} yet.")
  }
}

fn generation_message(theirs: u32, ours: u32) -> String {
  format!(
    "The latest herdr-core speaks protocol {theirs}, and this Visr speaks {ours}. \
     Update Visr to use it."
  )
}

/// What the list of releases says about this machine.
fn judge(listed: Result<Manifest, String>, os: &str, arch: &str) -> State {
  let manifest = match listed {
    Ok(manifest) => manifest,
    Err(message) => {
      return State::Failed {
        message,
        retry: Retry::Check,
      };
    }
  };
  match manifest::check(&manifest, PROTOCOL_GENERATION, os, arch) {
    Ok(plan) => State::Ready(plan),
    Err(Refusal::Generation { theirs, ours }) => {
      State::Unsupported(generation_message(theirs, ours))
    }
    Err(Refusal::NoBuild { os, arch }) => State::Unsupported(no_build_message(&os, &arch)),
  }
}

/// What the window says when herdr-core is in place and answering.
fn done_title(reason: Reason, version: Option<&str>) -> String {
  let what = match (reason, version) {
    (Reason::OutOfDate, Some(version)) => format!("herdr-core is updated to {version}"),
    (Reason::OutOfDate, None) => "herdr-core is updated".to_string(),
    (Reason::Missing, Some(version)) => format!("herdr-core {version} is installed"),
    (Reason::Missing, None) => "herdr-core is installed".to_string(),
  };
  format!("{what} and running")
}

fn done_message(reason: Reason) -> &'static str {
  match reason {
    Reason::Missing => {
      "Your terminals live in herdr-core, so they keep running even when you close Visr."
    }
    Reason::OutOfDate => "Your terminals kept running through the update.",
  }
}

/// A warning that the installer would come from somewhere other than herdr's own server.
fn override_warning(releases: &str) -> Option<String> {
  (!install::is_official_releases(releases)).then(|| {
    format!(
      "Not herdr's official server: VISR_HERDR_RELEASES_URL points to {releases}. \
       Only go on if you set that yourself."
    )
  })
}

/// What the person is told before anything runs.
fn details(plan: &Plan, windows: bool, releases: &str) -> Vec<String> {
  let host = if install::is_official_releases(releases) {
    releases
      .split_once("://")
      .map_or(releases, |(_, rest)| rest)
      .trim_end_matches('/')
      .to_string()
  } else {
    format!("{releases} (set by VISR_HERDR_RELEASES_URL)")
  };
  let place = if windows {
    "%LOCALAPPDATA%\\Programs\\herdr-core, added to your PATH"
  } else if cfg!(target_os = "macos") {
    "~/.local/bin (hc and herdr-core) and ~/.local/share/herdr-core (Herdr Core.app)"
  } else {
    "~/.local/bin (hc and herdr-core)"
  };
  let mut lines = vec![
    format!("Version {} for {}, from {host}.", plan.version, plan.target),
    format!("Installs into {place}."),
    "No administrator rights are used.".to_string(),
  ];
  if cfg!(target_os = "macos") {
    lines.push("The installer checks the build's code signature before installing it.".into());
  }
  lines.push("herdr-core is open source under the Apache 2.0 license.".into());
  lines
}

/// Saves the installer in a private folder, ready to run; the folder is gone again on an error.
/// Starting it is left to the caller, in the foreground: a task dropped while this downloads
/// (Cancel, closing the window) must not leave a program running that nothing can stop.
fn prepare(plan: &Plan) -> Result<(PathBuf, std::process::Command), String> {
  let windows = cfg!(windows);
  let releases = install::releases_url();
  let dir = install::private_dir().map_err(|error| format!("couldn't make a folder: {error}"))?;
  match install::download_script(&install::script_url(&releases, windows), &dir, windows) {
    Ok(script) => {
      let overridden = (!install::is_official_releases(&releases)).then_some(releases.as_str());
      Ok((
        dir,
        install::install_command(plan, windows, &script, overridden),
      ))
    }
    Err(message) => {
      let _ = std::fs::remove_dir_all(&dir);
      Err(message)
    }
  }
}

/// What the installer's exit says: nothing to report when it succeeded.
fn install_outcome(status: std::io::Result<ExitStatus>) -> Result<(), String> {
  match status {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => Err(match status.code() {
      Some(code) => format!("The installer stopped with code {code}."),
      None => "The installer was stopped.".to_string(),
    }),
    Err(error) => Err(format!("Couldn't tell how the installer ended: {error}")),
  }
}

/// A core just installed may not be running: if connecting fails for any reason but the program
/// being missing, it is started once (that does nothing to one already running), and the
/// connection tried again, with a moment for it to come up.
fn open_connection() -> Result<Connection, Error> {
  match Connection::open() {
    Ok(connection) => return Ok(connection),
    Err(error) if reason(&error) == Some(Reason::Missing) => return Err(error),
    Err(_) => {
      let _ = hc_client::run_hc(&["server", "start"]);
    }
  }
  let mut last = None;
  for attempt in 0..CONNECT_TRIES {
    if attempt > 0 {
      std::thread::sleep(Duration::from_secs(1));
    }
    match Connection::open() {
      Ok(connection) => return Ok(connection),
      Err(error) => last = Some(error),
    }
  }
  Err(last.expect("at least one attempt"))
}

impl Drop for Setup {
  /// Closing the window ends the install with it: the program and what it started, then its copy
  /// of the installer.
  fn drop(&mut self) {
    self.stop();
  }
}

impl Setup {
  pub(crate) fn new(
    reason: Reason,
    launch: Launch,
    window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let mut setup = Self {
      reason,
      state: State::Checking,
      log: Vec::new(),
      cancel: None,
      scratch: None,
      launch: Some(launch),
      installed: None,
      connection: None,
      focus_handle: cx.focus_handle(),
      entrance: Tween::settled(1.0),
      shown: None,
      leaving: None,
      _task: None,
    };
    setup.check(window, cx);
    setup
  }

  fn check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if install::running_as_root() {
      self.state = State::Unsupported(
        "Open Visr as yourself, not as an administrator: herdr-core installs for one person."
          .into(),
      );
      return;
    }
    self.state = State::Checking;
    self._task = Some(cx.spawn_in(window, async move |this, cx| {
      let listed = cx
        .background_executor()
        .spawn(async move { install::fetch_manifest(&install::releases_url()) })
        .await;
      let _ = this.update(cx, |setup, cx| {
        setup.state = judge(listed, std::env::consts::OS, std::env::consts::ARCH);
        cx.notify();
      });
    }));
    cx.notify();
  }

  fn install(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
    self.state = State::Installing(plan.clone());
    self.installed = None;
    self.log.clear();
    self.cancel = None;
    self._task = Some(cx.spawn_in(window, async move |this, cx| {
      let prepared = cx
        .background_executor()
        .spawn({
          let plan = plan.clone();
          async move { prepare(&plan) }
        })
        .await;
      let started = prepared.and_then(|(dir, command)| match install::run(command) {
        Ok(started) => Ok((dir, started)),
        Err(error) => {
          let _ = std::fs::remove_dir_all(&dir);
          Err(format!("couldn't run the installer: {error}"))
        }
      });
      let (dir, (events, cancel)) = match started {
        Ok(started) => started,
        Err(message) => {
          let _ = this.update(cx, |setup, cx| {
            setup.fail(message, Retry::Install(plan), cx);
          });
          return;
        }
      };
      let _ = this.update(cx, |setup, _| {
        setup.scratch = Some(dir);
        setup.cancel = Some(cancel);
      });
      loop {
        let mut lines = Vec::new();
        let mut finished = None;
        while let Ok(event) = events.try_recv() {
          match event {
            Event::Line(line) => lines.push(line),
            Event::Finished(status) => {
              finished = Some(status);
              break;
            }
          }
        }
        let done = finished.is_some();
        let plan = plan.clone();
        let updated = this.update_in(cx, |setup, window, cx| {
          setup.log.extend(lines);
          if setup.log.len() > LOG_KEPT {
            setup.log.drain(..setup.log.len() - LOG_KEPT);
          }
          match finished {
            Some(status) => {
              setup.cleanup();
              match install_outcome(status) {
                Ok(()) => {
                  setup.installed = Some(plan.version.clone());
                  setup.connect(window, cx);
                }
                Err(message) => setup.fail(message, Retry::Install(plan), cx),
              }
            }
            None => cx.notify(),
          }
        });
        if done || updated.is_err() {
          return;
        }
        cx.background_executor()
          .timer(Duration::from_millis(100))
          .await;
      }
    }));
    cx.notify();
  }

  /// Forgets the installer's copy and its running program.
  fn cleanup(&mut self) {
    self.cancel = None;
    if let Some(dir) = self.scratch.take() {
      let _ = std::fs::remove_dir_all(dir);
    }
  }

  /// Ends the program and what it started, then forgets it.
  fn stop(&mut self) {
    if let Some(cancel) = &self.cancel {
      cancel.cancel();
    }
    self.cleanup();
  }

  fn cancel_install(&mut self, plan: Plan, cx: &mut Context<Self>) {
    self._task = None;
    self.stop();
    self.fail(
      "Stopped. The installer may have got part of the way, and running it again picks up \
       from there."
        .into(),
      Retry::Install(plan),
      cx,
    );
  }

  fn fail(&mut self, message: String, retry: Retry, cx: &mut Context<Self>) {
    self.installed = None;
    self.state = State::Failed { message, retry };
    cx.notify();
  }

  fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.state = State::Connecting;
    self._task = Some(cx.spawn_in(window, async move |this, cx| {
      let opened = cx
        .background_executor()
        .spawn(async move { open_connection() })
        .await;
      let _ = this.update_in(cx, |setup, _, cx| match opened {
        Ok(connection) => {
          setup.connection = Some(connection);
          setup.state = State::Done;
          cx.notify();
        }
        Err(error) => setup.fail(startup_message(&error), Retry::Connect, cx),
      });
    }));
    cx.notify();
  }

  /// Swaps the window's content for the workspace, on the connection that was kept.
  fn open_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.leaving.is_some() || self.connection.is_none() {
      return;
    }
    let duration = motion::duration();
    if duration.is_zero() {
      self.enter(window, cx);
      return;
    }
    // The setup fades out first, and the app takes the window once it has.
    self.leaving = Some(Tween::new(0.0, 1.0, Instant::now(), duration));
    self._task = Some(cx.spawn_in(window, async move |this, cx| {
      cx.background_executor().timer(duration).await;
      let _ = this.update_in(cx, |setup, window, cx| setup.enter(window, cx));
    }));
    cx.notify();
  }

  fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let (Some(connection), Some(launch)) = (self.connection.take(), self.launch.take()) else {
      return;
    };
    window.replace_root(cx, |window, cx| {
      let mut workspace = Workspace::new(
        connection,
        launch.initial,
        launch.config,
        launch.config_error,
        launch.theme_notice,
        window,
        cx,
      );
      workspace.fade_in();
      workspace
    });
  }

  fn retry(&mut self, retry: Retry, window: &mut Window, cx: &mut Context<Self>) {
    match retry {
      Retry::Check => self.check(window, cx),
      Retry::Install(plan) => self.install(plan, window, cx),
      Retry::Connect => self.connect(window, cx),
    }
  }
}

fn button(id: &'static str, label: &str, primary: bool, colors: Colors) -> Stateful<gpui::Div> {
  div()
    .id(id)
    .px_4()
    .py_2()
    .rounded_md()
    .cursor_pointer()
    .bg(if primary {
      colors.button_primary
    } else {
      colors.surface_active
    })
    .hover(|button| button.opacity(0.85))
    .child(label.to_string())
}

fn column() -> gpui::Div {
  div().flex().flex_col().gap_4()
}

impl Setup {
  /// Enter or Space does what the screen's main button does, where there is one.
  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    // A held key repeats: someone holding Enter through a fast install must not land in the app.
    let modifiers = &event.keystroke.modifiers;
    if event.is_held
      || modifiers.control
      || modifiers.platform
      || !matches!(event.keystroke.key.as_str(), "enter" | "space")
    {
      return;
    }
    match &self.state {
      State::Ready(plan) => {
        let plan = plan.clone();
        self.install(plan, window, cx);
      }
      State::Done => self.open_app(window, cx),
      _ => {}
    }
  }
}

impl Render for Setup {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let now = Instant::now();
    // Each kind of screen comes in as it appears, unless the system asks for less motion.
    let kind = std::mem::discriminant(&self.state);
    if self.shown != Some(kind) {
      self.shown = Some(kind);
      self.entrance = Tween::new(0.0, 1.0, now, motion::duration());
    }
    let leaving = self
      .leaving
      .as_ref()
      .map_or(0.0, |leaving| leaving.value(now));
    let moving = !self.entrance.done(now) || self.leaving.as_ref().is_some_and(|l| !l.done(now));
    if moving {
      window.request_animation_frame();
    }
    let coming = self.entrance.value(now);
    // Where Enter has something to do, the window takes the keyboard.
    if matches!(self.state, State::Ready(_) | State::Done) && !self.focus_handle.is_focused(window)
    {
      window.focus(&self.focus_handle);
    }
    let colors = Colors::active(cx);
    let dim = colors.text_secondary;
    let button = |id: &'static str, label: &str, primary: bool| button(id, label, primary, colors);
    let link = |id: &'static str, label: &str, url: String| Link::new(id, label.to_string(), url);
    let heading = |text: &str| div().text_size(px(20.0)).child(text.to_string());
    let paragraph = |text: &str| div().text_color(dim).child(text.to_string());
    let manual = || link("manual", "Install it manually", MANUAL_URL.to_string());
    let log = |lines: &[String]| {
      let shown = &lines[lines.len().saturating_sub(LOG_LINES)..];
      div()
        .p_3()
        .rounded_md()
        .bg(colors.surface_sunken)
        .text_size(px(12.0))
        .text_color(dim)
        .children(shown.iter().map(|line| div().child(line.clone())))
    };

    let content: AnyElement = match &self.state {
      State::Checking => heading("Looking for herdr-core…").into_any_element(),
      State::Ready(plan) => {
        let (title, intro, action) = match self.reason {
          Reason::Missing => (
            "Visr needs herdr-core",
            "herdr-core runs your terminals and keeps them alive when you close Visr. Visr can \
             install it for you, with herdr-core's own installer.",
            "Install herdr-core",
          ),
          Reason::OutOfDate => (
            "herdr-core needs an update",
            "The herdr-core on this machine speaks a different protocol than Visr. Visr can \
             update the one you have, with herdr-core's own installer. Your running terminals \
             keep running.",
            "Update herdr-core",
          ),
        };
        let releases = install::releases_url();
        let script = install::script_url(&releases, cfg!(windows));
        let plan = plan.clone();
        column()
          .child(heading(title))
          .child(paragraph(intro))
          .children(override_warning(&releases).map(|warning| {
            div()
              .text_size(px(13.0))
              .text_color(colors.text_warning)
              .child(warning)
          }))
          .child(
            div().flex().flex_col().gap_1().children(
              details(&plan, cfg!(windows), &releases)
                .into_iter()
                .map(|line| div().text_size(px(13.0)).child(line)),
            ),
          )
          .child(
            div()
              .flex()
              .items_center()
              .gap_4()
              .child(button("install", action, true).on_click(
                cx.listener(move |setup, _, window, cx| setup.install(plan.clone(), window, cx)),
              ))
              .child(link("script", "View the installer", script))
              .child(manual()),
          )
          .into_any_element()
      }
      State::Installing(plan) => {
        let plan = plan.clone();
        column()
          .child(heading(&format!("Installing herdr-core {}…", plan.version)))
          .child(log(&self.log))
          .child(
            div()
              .flex()
              .child(button("cancel", "Cancel", false).on_click(
                cx.listener(move |setup, _, _, cx| setup.cancel_install(plan.clone(), cx)),
              )),
          )
          .into_any_element()
      }
      State::Connecting => heading("Starting herdr-core…").into_any_element(),
      State::Done => column()
        .child(heading(&done_title(self.reason, self.installed.as_deref())))
        .child(paragraph(done_message(self.reason)))
        .child(
          div().flex().child(
            button("open", "Open Visr", true)
              .on_click(cx.listener(|setup, _, window, cx| setup.open_app(window, cx))),
          ),
        )
        .into_any_element(),
      State::Failed { message, retry } => {
        let label = match retry {
          Retry::Connect => "Try connecting again",
          _ => "Try again",
        };
        let retry = retry.clone();
        let text = self.log.join("\n");
        let mut row =
          div()
            .flex()
            .items_center()
            .gap_4()
            .child(button("retry", label, true).on_click(
              cx.listener(move |setup, _, window, cx| setup.retry(retry.clone(), window, cx)),
            ));
        if !self.log.is_empty() {
          row = row.child(button("copy", "Copy log", false).on_click(cx.listener(
            move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())),
          )));
        }
        column()
          .child(heading("That didn't work"))
          .child(paragraph(message))
          .children((!self.log.is_empty()).then(|| log(&self.log)))
          .child(row.child(manual()))
          .into_any_element()
      }
      State::Unsupported(message) => column()
        .child(heading("herdr-core can't be installed from here"))
        .child(paragraph(message))
        .child(manual())
        .into_any_element(),
    };

    div()
      .id("setup")
      .track_focus(&self.focus_handle)
      .on_key_down(cx.listener(Self::on_key_down))
      .size_full()
      .flex()
      .items_center()
      .justify_center()
      .p_8()
      .bg(colors.surface)
      .text_color(colors.text)
      .text_size(px(15.0))
      .child(
        div()
          .w(px(540.0))
          .relative()
          .top(px(10.0 * (1.0 - coming)))
          .opacity(coming * (1.0 - leaving))
          .child(content),
      )
  }
}
