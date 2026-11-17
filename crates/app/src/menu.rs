//! Quit as a GPUI action, so its key works in every window, and the macOS app menu that shows it.

use config::{Action, Keymap};
use gpui::{App, KeyBinding, Menu, MenuItem, actions};

use crate::{about, input};

actions!(visr, [About, Quit]);

/// Registers the Quit action once and binds its key, which is always the platform default, so a
/// config reload never rebinds it. The setup screen is not under the workspace, so on every
/// platform this is what makes the key work there.
pub(crate) fn install(keys: &Keymap, cx: &mut App) {
  cx.on_action(|_: &Quit, cx| cx.quit());
  let press = keys
    .binding(Action::Quit)
    .expect("quit always has its platform key");
  cx.bind_keys([KeyBinding::new(&input::gpui_keystroke(press), Quit, None)]);
  // The menu is set after the binding because it reads the shortcut from the bindings.
  if cfg!(target_os = "macos") {
    cx.on_action(|_: &About, cx| about::open(cx));
    cx.set_menus(vec![Menu {
      name: "Visr".into(),
      items: vec![
        MenuItem::action("About Visr", About),
        MenuItem::separator(),
        MenuItem::action("Quit Visr", Quit),
      ],
    }]);
  }
}
