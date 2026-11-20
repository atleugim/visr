//! The user's font, theme choice and keys: built-in defaults with an optional TOML file on top, so
//! the same file gives the same app on every machine.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use theme::{Appearance, Theme};
use toml::{Table, Value};

const COMMON_DEFAULTS: &str = include_str!("../defaults/common.toml");
#[cfg(target_os = "macos")]
const PLATFORM_DEFAULTS: &str = include_str!("../defaults/macos.toml");
#[cfg(not(target_os = "macos"))]
const PLATFORM_DEFAULTS: &str = include_str!("../defaults/other.toml");

/// Why a config could not be loaded, as a message ready to show to the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for ConfigError {}

/// The settings Visr runs with: font, which theme to use and keybindings.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
  pub font: FontConfig,
  pub theme: ThemeChoice,
  pub keys: Keymap,
  /// What was left out of the file, ready to show to the person. Empty when all of it applied.
  pub messages: Vec<String>,
}

/// Which theme the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeChoice {
  /// No `theme` line: follow the system appearance.
  System,
  /// The theme named in `config.toml`, whatever the system appearance is.
  Named(String),
}

/// The terminal font. Sizes are in logical pixels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontConfig {
  pub family: String,
  pub size: f32,
  pub line_height: f32,
}

/// The merged TOML as read, before bindings are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
  font: FontConfig,
  theme: Option<String>,
  keys: BTreeMap<String, String>,
}

impl Config {
  /// The built-in defaults for this platform.
  pub fn defaults() -> Self {
    Self::parse("").expect("the built-in defaults are valid")
  }

  /// The defaults with the user's TOML on top.
  fn parse(user: &str) -> Result<Self, ConfigError> {
    let mut user_table = parse_table(user)?;
    // Removed before the merge so a value of any type can't fail the parse, and noted for the
    // message.
    let quit_ignored = match user_table.get_mut("keys").and_then(Value::as_table_mut) {
      Some(keys) => keys.remove("quit").is_some(),
      None => false,
    };
    let user_keys: Vec<String> = user_table
      .get("keys")
      .and_then(Value::as_table)
      .map(|keys| keys.keys().cloned().collect())
      .unwrap_or_default();
    let platform_table = parse_table(PLATFORM_DEFAULTS)?;
    let platform_keys = platform_table
      .get("keys")
      .and_then(Value::as_table)
      .expect("the platform defaults have keys");
    let mut merged = parse_table(COMMON_DEFAULTS)?;
    merge(&mut merged, platform_table.clone());
    merge(&mut merged, user_table);
    let raw: RawConfig = Value::Table(merged)
      .try_into()
      .map_err(|error| ConfigError(error.to_string()))?;
    let (keys, messages) = Keymap::from_names(&raw.keys, &user_keys, quit_ignored, platform_keys)?;
    Ok(Self {
      font: raw.font,
      theme: raw.theme.map_or(ThemeChoice::System, ThemeChoice::Named),
      keys,
      messages,
    })
  }

  /// A missing file is fine: the defaults apply.
  pub fn load(path: &Path) -> Result<Self, ConfigError> {
    match std::fs::read_to_string(path) {
      Ok(text) => Self::parse(&text).map(|mut config| {
        for message in &mut config.messages {
          *message = format!("In {}, {message}", path.display());
        }
        config
      }),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::defaults()),
      Err(error) => Err(ConfigError(format!("{}: {error}", path.display()))),
    }
  }
}

/// `VISR_CONFIG`, else `~/.config/visr/config.toml`, or `%APPDATA%\visr\config.toml` on Windows.
pub fn config_path() -> PathBuf {
  if let Some(path) = std::env::var_os("VISR_CONFIG") {
    return path.into();
  }
  let base = if cfg!(windows) {
    std::env::var_os("APPDATA").map(PathBuf::from)
  } else {
    std::env::home_dir().map(|home| home.join(".config"))
  };
  base.unwrap_or_default().join("visr").join("config.toml")
}

/// The folder of the user's themes: `themes/` next to the config file.
pub fn themes_dir() -> PathBuf {
  config_path().with_file_name("themes")
}

/// A theme ready to draw with, and what the person should be told about how it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTheme {
  pub theme: Theme,
  /// One entry per problem, in full sentences, some with a detail line after them. Empty when
  /// the theme loaded cleanly.
  pub messages: Vec<String>,
}

impl LoadedTheme {
  /// Reads the theme `choice` asks for. `system` picks the built-in one for `ThemeChoice::System`.
  ///
  /// A named theme is looked up in `themes_dir` first, then among the built-ins. One that is
  /// missing or can't be parsed gives `default` and a message. Reads a file, so keep it off the
  /// render path.
  pub fn load(choice: &ThemeChoice, system: Appearance, themes_dir: &Path) -> Self {
    match choice {
      ThemeChoice::System => Self::from_builtin(system),
      ThemeChoice::Named(name) => Self::named(name, themes_dir),
    }
  }

  fn named(name: &str, themes_dir: &Path) -> Self {
    let path = themes_dir.join(format!("{name}.toml"));
    let is_unusable_name = name.is_empty() || name.contains(['/', '\\']) || name.contains("..");
    if is_unusable_name {
      return Self::missing(name, themes_dir);
    }
    match std::fs::read_to_string(&path) {
      Ok(text) => match theme::parse(&text) {
        Ok(parsed) => Self::from_parsed(parsed, &path.display().to_string()),
        Err(error) => Self::fallback(format!(
          "The theme file {} is not valid TOML. Using the default theme.\n{error}",
          path.display()
        )),
      },
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => match theme::builtin(name) {
        Some(parsed) => Self::from_parsed(parsed, &format!("the built-in theme `{name}`")),
        None => Self::missing(name, themes_dir),
      },
      Err(error) => Self::fallback(format!(
        "The theme file {} can't be opened: {error}. Using the default theme.",
        path.display()
      )),
    }
  }

  fn missing(name: &str, themes_dir: &Path) -> Self {
    Self::fallback(format!(
      "There is no theme called `{name}`: add it to {} or pick a built-in one. \
       Using the default theme.",
      themes_dir.display()
    ))
  }

  /// A built-in theme, with its issues named after its own file.
  fn from_builtin(appearance: Appearance) -> Self {
    let name = match appearance {
      Appearance::Dark => "default",
      Appearance::Light => "default_light",
    };
    Self::from_parsed(
      theme::default_theme(appearance),
      &format!("the built-in theme `{name}`"),
    )
  }

  fn from_parsed(parsed: theme::ParsedTheme, source: &str) -> Self {
    let messages = parsed
      .issues
      .iter()
      .map(|issue| format!("In {source}, {issue}."))
      .collect();
    Self {
      theme: parsed.theme,
      messages,
    }
  }

  fn fallback(message: String) -> Self {
    let mut loaded = Self::from_builtin(Appearance::Dark);
    loaded.messages.insert(0, message);
    loaded
  }
}

fn parse_table(text: &str) -> Result<Table, ConfigError> {
  text
    .parse::<Table>()
    .map_err(|error| ConfigError(error.to_string()))
}

/// Tables merge key by key; anything else in `over` replaces what `base` had.
fn merge(base: &mut Table, over: Table) {
  for (key, value) in over {
    match (base.get_mut(&key), value) {
      (Some(Value::Table(base_table)), Value::Table(over_table)) => merge(base_table, over_table),
      (_, value) => {
        base.insert(key, value);
      }
    }
  }
}

/// A key as pressed: the key's name (`t`, `]`, `enter`) and exactly which modifiers were held.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPress {
  pub key: String,
  pub ctrl: bool,
  pub alt: bool,
  pub shift: bool,
  /// cmd on macOS, the Windows key elsewhere.
  pub platform: bool,
}

impl KeyPress {
  /// How menus write the shortcut on this platform: `⌥⌘W` on macOS, `Alt+Win+W` elsewhere.
  pub fn label(&self) -> String {
    let mac = cfg!(target_os = "macos");
    let key = match (self.key.as_str(), mac) {
      ("up", true) => "↑".to_string(),
      ("down", true) => "↓".to_string(),
      ("left", true) => "←".to_string(),
      ("right", true) => "→".to_string(),
      (key, true) => key.to_uppercase(),
      (key, false) => {
        let mut chars = key.chars();
        chars
          .next()
          .map(|first| first.to_uppercase().chain(chars).collect())
          .unwrap_or_default()
      }
    };
    let modifiers = [
      (self.ctrl, "⌃", "Ctrl"),
      (self.alt, "⌥", "Alt"),
      (self.shift, "⇧", "Shift"),
      (self.platform, "⌘", "Win"),
    ];
    let held = modifiers
      .iter()
      .filter(|(held, _, _)| *held)
      .map(|(_, symbol, word)| if mac { *symbol } else { *word });
    if mac {
      held.chain([key.as_str()]).collect()
    } else {
      held.chain([key.as_str()]).collect::<Vec<_>>().join("+")
    }
  }
}

/// Something a keybinding can ask the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
  NewTab,
  SplitRight,
  SplitDown,
  ClosePane,
  CloseTab,
  FocusNext,
  FocusPrev,
  /// Zero-based.
  SelectTab(usize),
  MoveTabUp,
  MoveTabDown,
  MoveToNewFolder,
  ToggleSidebar,
  ZoomPane,
  Copy,
  Paste,
  ReloadTheme,
  Quit,
}

fn action_named(name: &str) -> Option<Action> {
  let action = match name {
    "new_tab" => Action::NewTab,
    "split_right" => Action::SplitRight,
    "split_down" => Action::SplitDown,
    "close_pane" => Action::ClosePane,
    "close_tab" => Action::CloseTab,
    "focus_next" => Action::FocusNext,
    "focus_prev" => Action::FocusPrev,
    "move_tab_up" => Action::MoveTabUp,
    "move_tab_down" => Action::MoveTabDown,
    "move_to_new_folder" => Action::MoveToNewFolder,
    "toggle_sidebar" => Action::ToggleSidebar,
    "zoom_pane" => Action::ZoomPane,
    "copy" => Action::Copy,
    "paste" => Action::Paste,
    "reload_theme" => Action::ReloadTheme,
    "quit" => Action::Quit,
    _ => match name.strip_prefix("select_tab_")?.parse() {
      Ok(tab @ 1..=9) => Action::SelectTab(tab - 1),
      _ => return None,
    },
  };
  Some(action)
}

/// Parses `cmd-shift-d`, `ctrl--`, `alt-enter`.
fn parse_binding(text: &str) -> Result<KeyPress, ConfigError> {
  // GPUI splits a binding on whitespace into a key sequence, which the keymap can't match.
  if text.contains(char::is_whitespace) {
    return Err(ConfigError(format!(
      "keys: `{text}` has whitespace, which a key can't contain"
    )));
  }
  let (modifiers, key) = match text.strip_suffix("--") {
    Some(modifiers) => (modifiers, "-"),
    None => text.rsplit_once('-').unwrap_or(("", text)),
  };
  if key.is_empty() {
    return Err(ConfigError(format!("keys: `{text}` has no key")));
  }
  let mut press = KeyPress {
    key: key.to_lowercase(),
    ctrl: false,
    alt: false,
    shift: false,
    platform: false,
  };
  for modifier in modifiers.split('-').filter(|modifier| !modifier.is_empty()) {
    match modifier {
      "cmd" | "super" | "win" => press.platform = true,
      "ctrl" => press.ctrl = true,
      "alt" | "opt" | "option" => press.alt = true,
      "shift" => press.shift = true,
      _ => {
        return Err(ConfigError(format!(
          "keys: unknown modifier `{modifier}` in `{text}`"
        )));
      }
    }
  }
  Ok(press)
}

/// The bindings from keys to actions, built from the defaults and the user's `[keys]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Keymap {
  bindings: Vec<(KeyPress, Action)>,
}

/// An action whose binding was ignored because it used the quit key, and so fell back to its
/// default.
struct Restored<'a> {
  name: &'a str,
  /// The start of the message to show: what was ignored and why.
  ignored: String,
  default: &'a str,
  press: KeyPress,
}

impl Keymap {
  /// `user_keys` are the actions the user's file sets: their keys are theirs, so a default on the
  /// same key gives way. Two bindings left on one key is an error, not a silent winner. `quit`
  /// can't be changed and no other action may use its key. Neither is an error: the binding is
  /// ignored and the default stays, with a message for the person. `quit_ignored` says the user's
  /// file set `quit`, which `names` no longer carries. The messages come back with the keymap.
  fn from_names(
    names: &BTreeMap<String, String>,
    user_keys: &[String],
    quit_ignored: bool,
    platform_keys: &Table,
  ) -> Result<(Self, Vec<String>), ConfigError> {
    let default_binding = |name: &str| platform_keys.get(name).and_then(Value::as_str);
    let quit_binding = default_binding("quit").expect("the platform defaults bind quit");
    let quit_press = parse_binding(quit_binding).expect("the built-in quit key is valid");
    let mut messages = Vec::new();
    if quit_ignored {
      messages.push(format!(
        "keys: `quit` can't be changed, so that binding was ignored and its default \
         `{quit_binding}` is used."
      ));
    }
    // Actions that fell back to their default: that key gives way to the user's bindings.
    let mut restored: Vec<Restored> = Vec::new();
    let mut parsed = Vec::new();
    for (name, binding) in names {
      let action =
        action_named(name).ok_or_else(|| ConfigError(format!("keys: unknown action `{name}`")))?;
      if !binding.is_empty() {
        let mut press = parse_binding(binding)?;
        if action != Action::Quit && press == quit_press {
          let ignored = format!("keys: `{name}` can't use `{binding}`, which is kept for quit");
          let default = default_binding(name).expect("every action has a platform default");
          press = parse_binding(default).expect("the built-in defaults are valid");
          restored.push(Restored {
            name,
            ignored,
            default,
            press: press.clone(),
          });
        }
        parsed.push((name.as_str(), press, action));
      }
    }
    let is_user_key = |name: &str| {
      user_keys.iter().any(|key| key == name) && !restored.iter().any(|r| r.name == name)
    };
    let user_presses: Vec<&KeyPress> = parsed
      .iter()
      .filter(|(name, _, _)| is_user_key(name))
      .map(|(_, press, _)| press)
      .collect();
    let kept: Vec<&(&str, KeyPress, Action)> = parsed
      .iter()
      .filter(|(name, press, _)| is_user_key(name) || !user_presses.contains(&press))
      .collect();
    for (index, (name, press, _)) in kept.iter().enumerate() {
      if let Some((other, _, _)) = kept[..index].iter().find(|(_, other, _)| other == press) {
        return Err(ConfigError(format!(
          "keys: `{other}` and `{name}` are both bound to the same key"
        )));
      }
    }
    // A restored default can still lose to a user's binding, so say what really happened.
    for Restored {
      name,
      ignored,
      default,
      press: restored_press,
    } in &restored
    {
      let taker = kept
        .iter()
        .find(|(n, press, _)| n != name && press == restored_press)
        .map(|(n, _, _)| *n);
      messages.push(match taker {
        None => {
          format!("{ignored}, so that binding was ignored and its default `{default}` is used.")
        }
        Some(other) => format!(
          "{ignored}, so that binding was ignored. Its default `{default}` is taken by `{other}`, \
           so it has no key."
        ),
      });
    }
    let bindings = kept
      .into_iter()
      .map(|(_, press, action)| (press.clone(), *action))
      .collect();
    Ok((Self { bindings }, messages))
  }

  /// The key bound to `action`, if it has one.
  pub fn binding(&self, action: Action) -> Option<&KeyPress> {
    self
      .bindings
      .iter()
      .find(|(_, bound)| *bound == action)
      .map(|(press, _)| press)
  }

  /// The action bound to exactly this key press, if any.
  pub fn action(&self, press: &KeyPress) -> Option<Action> {
    self
      .bindings
      .iter()
      .find(|(bound, _)| bound == press)
      .map(|(_, action)| *action)
  }
}
