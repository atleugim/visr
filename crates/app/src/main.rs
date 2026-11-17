//! The `visr` binary: opens the window and wires the app together.

// Release builds on Windows open no console window next to the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod about;
mod assets;
mod connection;
mod editors;
mod git;
mod input;
mod menu;
mod setup;
mod startup;
mod terminal_view;
mod tree;
mod workspace;

use std::borrow::Cow;

use config::{Config, LoadedTheme};
use connection::Connection;
use gpui::{
  App, Application, Bounds, SharedString, TitlebarOptions, WindowAppearance, WindowBounds,
  WindowOptions, point, prelude::*, px, size,
};
use setup::{Launch, Setup};
use startup::{StartupError, startup_message};
use terminal_view::TerminalColors;
use theme::Appearance;
use ui::tokens::{TRAFFIC_LIGHTS_LEFT, TRAFFIC_LIGHTS_TOP};
use workspace::Workspace;

const SIDEBAR_WIDTH: f32 = 220.0;

/// The default font ships inside the binary, so every machine renders the same without installing
/// anything. Licensed under the SIL OFL 1.1: see assets/fonts/LICENSE.txt, and NOTICE.txt for the
/// Nerd Fonts glyphs.
const FONTS: [&[u8]; 4] = [
  include_bytes!("../assets/fonts/LilexNerdFontMono-Regular.ttf"),
  include_bytes!("../assets/fonts/LilexNerdFontMono-Bold.ttf"),
  include_bytes!("../assets/fonts/LilexNerdFontMono-Italic.ttf"),
  include_bytes!("../assets/fonts/LilexNerdFontMono-BoldItalic.ttf"),
];

fn main() {
  ui::motion::check_reduce_motion();
  let connection = Connection::open();
  let initial = std::env::args().nth(1);
  let (config, config_error) = match Config::load(&config::config_path()) {
    Ok(config) => (config, None),
    Err(error) => (Config::defaults(), Some(error.to_string())),
  };

  Application::new()
    .with_assets(assets::Assets)
    .run(move |cx: &mut App| {
      let fonts = FONTS.iter().map(|font| Cow::Borrowed(*font)).collect();
      cx.text_system()
        .add_fonts(fonts)
        .expect("load the bundled fonts");
      menu::install(&config.keys, cx);
      cx.on_window_closed(|cx| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();
      // The theme is in place before any window exists, so the first frame already draws with it.
      let loaded = LoadedTheme::load(
        &config.theme,
        system_appearance(cx.window_appearance()),
        &config::themes_dir(),
      );
      ui::colors::install(&loaded.theme.ui, cx);
      cx.set_global(TerminalColors::new(&loaded.theme.terminal));
      // Shown once, when the window opens.
      let theme_notice = notice(&config.messages, &loaded.messages);
      let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
      let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
          title: Some("Visr".into()),
          // Windows keeps its native title bar: drawing our own needs the caption buttons too.
          appears_transparent: cfg!(target_os = "macos"),
          traffic_light_position: Some(point(px(TRAFFIC_LIGHTS_LEFT), px(TRAFFIC_LIGHTS_TOP))),
        }),
        ..Default::default()
      };
      let opened = match connection {
        Ok(connection) => cx
          .open_window(options, |window, cx| {
            cx.new(|cx| {
              Workspace::new(
                connection,
                initial,
                config,
                config_error,
                theme_notice,
                window,
                cx,
              )
            })
          })
          .map(|_| ()),
        // Missing or out of date, herdr-core is something Visr can install.
        Err(error) => match setup::reason(&error) {
          Some(reason) => {
            let launch = Launch {
              initial,
              config,
              config_error,
              theme_notice,
            };
            cx.open_window(options, |window, cx| {
              cx.new(|cx| Setup::new(reason, launch, window, cx))
            })
            .map(|_| ())
          }
          None => {
            let message = startup_message(&error);
            cx.open_window(options, |_, cx| cx.new(|_| StartupError { message }))
              .map(|_| ())
          }
        },
      };
      opened.expect("open the window");
      cx.activate(true);
    });
}

/// Which built-in theme the system appearance asks for when `config.toml` names none.
pub(crate) fn system_appearance(appearance: WindowAppearance) -> Appearance {
  match appearance {
    WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
    WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
  }
}

/// The banner text for what the config file and the theme left out, or `None` when all applied.
pub(crate) fn notice(config: &[String], theme: &[String]) -> Option<SharedString> {
  let messages: Vec<&str> = config.iter().chain(theme).map(String::as_str).collect();
  (!messages.is_empty()).then(|| messages.join("\n").into())
}
