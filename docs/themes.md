# Themes

Visr's look is driven by themes. A theme is a TOML file that sets every color of the chrome and of
the terminals. Changing the theme never needs a rebuild.

## Choosing a theme

The `theme` setting in `config.toml` decides which theme Visr uses:

```toml
theme = "tokyo_night"
```

- **Named.** With a `theme` line, that theme is used whatever the system appearance is.
- **System.** Without a `theme` line, Visr follows the system appearance: `default` when it is
  dark and `default_light` when it is light. It switches live when the system changes.

The name is the file name without `.toml`. Visr looks for `<config dir>/themes/<name>.toml` first,
then among the built-in themes, so a user file with a built-in's name replaces it. The config
folder is the one holding `config.toml`: `~/.config/visr` on macOS and Linux, `%APPDATA%\visr` on
Windows, or the folder of the file `VISR_CONFIG` points at.

The built-in themes are `default` (dark) and `default_light` (light).

### Applying changes

Visr reads `config.toml` again as soon as it is saved (or, if Visr could not watch its folder,
when the window becomes active), and a changed `theme` line applies then. Visr does not watch the
files in `themes/`. After editing a theme file, press the `reload_theme` shortcut. By default it is
`cmd-shift-r` on macOS and `ctrl-shift-r` elsewhere, and `reload_theme` under `[keys]` in
`config.toml` changes it. While no `theme` line is set, the system appearance is followed on its
own.

## The file

A theme file has three tables.

```toml
[meta]
name = "My theme"
appearance = "dark"

[ui]
# chrome roles

[terminal]
# terminal colors
```

Colors are strings, `#rrggbb` or `#rrggbbaa`.

### `[meta]`

| Key          | Meaning                                                                          |
| ------------ | -------------------------------------------------------------------------------- |
| `name`       | A label for the theme. The file name is what `theme` refers to.                  |
| `appearance` | `"dark"` or `"light"`. Chooses the built-in theme that fills in missing roles.   |

### `[ui]`

The roles the chrome draws with. Only the chrome that sits on the terminal background (the
focused pane's title, the pane body, the startup screen) takes its colors from `[terminal]`.

| Role                   | Where it shows                                                                                                  |
| ---------------------- | --------------------------------------------------------------------------------------------------------------- |
| `surface`              | The window and the sidebar behind everything else.                                                              |
| `surface_hover`        | A row or a button under the pointer.                                                                            |
| `surface_active`       | The active row, an icon's backing and a secondary button.                                                       |
| `surface_hover_strong` | A button under the pointer inside a row that is itself hovered or active, and the edge and the separators of a menu. |
| `surface_raised`       | Menus and other panels that float over the window.                                                              |
| `surface_raised_hover` | A menu row under the pointer.                                                                                   |
| `surface_sunken`       | A recessed area, such as the setup log box.                                                                     |
| `surface_error`        | The banner that reports an error.                                                                               |
| `border`               | The edge of a pane and the divider lines.                                                                      |
| `border_focused`       | The edge of the focused pane, the sidebar's rename field, the back layer of the stack icon, a divider being hovered or dragged, and where a dragged sidebar line will land. |
| `border_snapped`       | A panel's edge while its width is the default one.                                                              |
| `text`                 | The main text of the chrome, and the cursor of a text field.                                                    |
| `text_muted`           | Labels, icons and shortcuts that stay quieter than the main text.                                               |
| `text_secondary`       | Secondary text in the setup window.                                                                             |
| `text_link`            | A link.                                                                                                         |
| `text_warning`         | A warning message.                                                                                              |
| `selection`            | The selected text of a text field.                                                                              |
| `button_primary`       | The background of a primary button.                                                                             |
| `git_modified`         | A modified file in the changes panel.                                                                           |
| `git_added`            | An added file in the changes panel.                                                                             |
| `git_deleted`          | A deleted file in the changes panel.                                                                            |
| `git_untracked`        | A renamed or untracked file in the changes panel.                                                               |
| `git_conflict`         | A file with a merge conflict in the changes panel.                                                              |
| `status_quiet`         | The dot of a terminal that has nothing going on.                                                                |
| `status_working`       | The dot of a terminal that is working.                                                                          |
| `status_blocked`       | The dot of a terminal that needs a person.                                                                      |
| `status_done`          | The dot of a terminal that is done.                                                                             |
| `status_exited`        | The dot of a terminal whose program has ended.                                                                  |

### `[terminal]`

The colors the terminal grids draw with. The grids use only these.

| Key          | Where it shows                                                                  |
| ------------ | ------------------------------------------------------------------------------- |
| `foreground` | The text of the grid, and the chrome around it that sits on the terminal background. |
| `background` | The grid, the pane body and the startup screen.                                 |
| `cursor`     | The cursor of the grid.                                                         |
| `selection`  | The selected text of the grid.                                                  |
| `ansi`       | A list of exactly 16 colors: normal 0-7 (black, red, green, yellow, blue, magenta, cyan, white), then bright 8-15 in the same order. |

Terminal colors are always drawn opaque. An `#rrggbbaa` value is accepted, but its alpha is
ignored. In `[ui]`, alpha is honored.

## Partial themes and errors

A theme may set only some roles.

- Every missing role is filled from the built-in theme of the same `appearance`, so a partial
  theme always renders. Without `appearance`, the theme is treated as dark.
- A missing `name` or `appearance` in `[meta]` is reported. An `appearance` other than `dark` or
  `light` is reported too, and treated as dark.
- An invalid color falls back to the built-in value for that role. In `ansi`, each bad entry falls
  back on its own, while a list that does not have 16 colors falls back whole.
- Unknown keys are ignored.
- Each of these is reported once, as a message that names the file and the key. The rest of the
  theme still applies.
- A name with no theme file and no built-in theme, or a file that is not valid TOML, falls back to
  `default` with a message.

## Example

A complete theme. It sets every role, with the values of the built-in `default`. Save it as
`<config dir>/themes/my_theme.toml` and set `theme = "my_theme"`.

```toml
[meta]
name = "My theme"
appearance = "dark"

[ui]
surface = "#141516"
surface_hover = "#222427"
surface_active = "#2a2d31"
surface_hover_strong = "#3a3d42"
surface_raised = "#232528"
surface_raised_hover = "#34373b"
surface_sunken = "#1c1e21"
surface_error = "#5c2b2b"
border = "#2a2c2f"
border_focused = "#44484d"
border_snapped = "#3b82f6"
text = "#c5c8c6"
text_muted = "#808080"
text_secondary = "#909090"
text_link = "#7aa2f7"
text_warning = "#e2a04a"
selection = "#373b41"
button_primary = "#3b6fd4"
git_modified = "#e2c08d"
git_added = "#81b88b"
git_deleted = "#c74e39"
git_untracked = "#73c991"
git_conflict = "#e4676b"
status_quiet = "#666666"
status_working = "#f0c674"
status_blocked = "#cc6666"
status_done = "#b5bd68"
status_exited = "#373b41"

[terminal]
foreground = "#c5c8c6"
background = "#1d1f21"
cursor = "#c5c8c6"
selection = "#373b41"
ansi = [
  "#1d1f21", "#cc6666", "#b5bd68", "#f0c674", "#81a2be", "#b294bb", "#8abeb7", "#c5c8c6",
  "#666666", "#d54e53", "#b9ca4a", "#e7c547", "#7aa6da", "#c397d8", "#70c0b1", "#eaeaea",
]
```
