# visr

A native desktop app for the terminals that herdr-core runs.

herdr-core (`hc`) owns the terminals: it starts the programs, keeps their output and history, and
reports their status (working, blocked, done). Visr is the window onto them. It draws the
terminals, lays them out in tabs and splits, and shows at a glance which ones need a person.
Closing a pane only stops showing a terminal. The terminal lives on in the core until it ends or is
killed, and the layout itself is saved in the core.

Built with Rust and [GPUI](https://github.com/zed-industries/zed), Zed's UI framework. Cargo
workspace, edition 2024, resolver 3. Targets macOS and Windows; CI and releases cover both.

## Core principles

- **Correct, then simple, then fast, and all three before handing off.** A change isn't done
  until it is right on all platforms, reads clearly, and costs nothing it doesn't need to.
  Review your own diff once more before calling it finished.
- **Smallest change that solves the problem.** No speculative abstractions, options, or error
  handling for cases that can't happen. Don't abstract before the third use. No drive-by
  refactors. Mention problems you notice outside the task, and leave them for their own change.
- **Performance is a feature.** Visr sits in front of programs people watch all day. Rendering,
  input, and terminal output are hot paths: no blocking I/O, no per-frame allocations you can
  avoid, no work repeated every frame that could be done once. Measure before you optimize
  anything else.
- **Keep the user in control.** Shortcuts always work. Motion respects the system setting. A
  terminal is never killed or hidden without the person asking.
- **Clarity over cleverness.** Name things after what they mean, not how they work.

## Crate layout

```
crates/
  app/        binary `visr`: window, workspace (sidebar, panes, toolbar, panels), setup, startup
  ui/         the design system: tokens, typography, icons, buttons, menus, text input, motion
  theme/      the theme file format: parsing, validation, fallback, and the built-in themes
  app-state/  what the app knows about the core's terminals, and the tabs-and-splits layout model
  config/     built-in defaults per OS plus the user's `config.toml`: font, keybindings, theme name
  hc-client/  blocking client for herdr-core over one framed connection (std threads + channels)
  hc-proto/   the wire format: frame codec, JSON-RPC, and the params and results we use
  term/       one terminal's screen over alacritty_terminal: grid, palette, keys, mouse
```

Dependency direction is strict. **NEVER** create a back edge:

```
app → ui, theme, app-state, config, hc-client, hc-proto, term, gpui
ui → theme, gpui
config → theme, term
app-state → hc-proto
hc-client → hc-proto
term → alacritty_terminal
```

- **Only `app` and `ui` depend on `gpui`.** Every other crate is plain Rust that builds without
  a window. If a library crate seems to need a GPUI type, the logic belongs in `app`, or the type
  becomes a plain struct that `app` converts.
- **`ui` is the design system and knows nothing else.** It has no idea that the core, the config
  or its files exist. It turns the active `theme::Theme` into GPUI values, and owns the fixed
  tokens (spacing, radii, font sizes, durations), every shared component, the icon set and the
  motion module. Components take plain data and callbacks, never app state. `app` builds screens
  out of `ui` and never styles from scratch.
- **`theme` is data, not drawing.** It defines what a theme file contains, parses and validates
  it, fills gaps from the built-in default, and ships the built-in themes. It does no file
  lookup (that is `config`'s job) and knows no GPUI types: colors are plain RGBA.
- **Only `term` names `alacritty_terminal`.** Other crates go through `term`'s own types and never
  see alacritty's.
- **`hc-proto` does no I/O.** It encodes, decodes and declares types. `hc-client` does the I/O and
  runs no async runtime: it uses threads and channels, so any executor can drive it.
- **`app-state` is pure models.** No I/O, no clock, no GPUI. It applies core events and edits the
  layout, and that's where layout and terminal-list logic lives.
- **`app` is the only place that touches the platform**: windows, focus, clipboard, child
  processes, and installing herdr-core.

### When to add a crate

Split a crate out of `app` when its code has a clear owner and a second consumer, not before.
The next expected splits:

- `views`: workspace, sidebar, panes, panels, setup. `app` keeps `main`, the window and the
  wiring. The graph becomes `app → views → ui`, with `views` on `app-state`, `config`,
  `hc-client` and `term`.
- External integrations (editors, git, installers) each start as a module in `app` with no GPUI
  in it, and move into their own crate when a second crate needs them.

## Architecture rules

- **The core is the source of truth.** Terminals, their status and the saved layout come from
  herdr-core's event stream. Don't keep a second copy that can drift. Change things through the
  core's methods and let the event come back.
- **Protocol types declare only what we read.** Unknown fields are skipped, so new core fields
  don't break us. Types are written from traffic seen from a real herdr-core. **NEVER** declare
  fields or messages you haven't seen. The protocol generation changes only
  together with the core.
- **Never block the UI on the core for long.** Client calls block and time out. A single call
  made directly from a key press or click is acceptable. Anything repeated, slow, or done at
  startup runs on the background executor, and its result is applied back in an entity update.
  Streams are awaited, never polled in a loop. **NEVER** call the core or touch the filesystem
  from `render`.
- **Store every task.** A spawned `Task` lives in a field of the entity that started it, so
  dropping the entity cancels it. **NEVER** `detach()` work that touches state. Anything a cancel
  must stop, like a child process, starts in the foreground continuation, not inside a background
  future that keeps running after its task is dropped.
- **Reuse before you build.** Shared elements, icons, menus, text input, theme values and motion
  live in `ui`, one place each. Look there first, and extend what exists instead of writing a
  sibling. A component used once can live in `app`; on its second use it moves to `ui`.
- **No hardcoded colors.** Every color, in the chrome and in the terminals, comes from the active
  theme (see Themes). Code names a color by its role, never by its value. Sizes that other code has to line up with (title bar, pane edges,
  title bar buttons) are named constants in `ui` derived from each other, never a second literal
  that happens to match. A literal size is fine for a local detail nothing else lines up with.
- **Motion goes through `ui::motion`.** Every animation uses its tween and its duration, which is
  zero when the system asks for reduced motion. While something moves, terminal grids keep their
  size and resize once at the end. Request animation frames only while a tween runs.
- **Keyboard first.** Every action has a keybinding in `config`, works whether or not a terminal
  is open, and shows its configured shortcut wherever the UI names it. Shortcuts never reach the
  terminal program. A focused text field keeps its own keys.
- **Every platform builds clean.** CI runs clippy with `-D warnings` on macOS and Windows. Code
  under `#[cfg(…)]` or `cfg!(…)` must leave nothing unused on the other platforms. Keep
  platform-specific behavior in small functions around a platform-free core.
- **Generated files are regenerated, never edited.** A generated file says so in its header and
  names its script under `scripts/`. Bundled fonts and icon packs ship with their license.

## UI guidelines

Visr is a tool people keep open all day next to their terminals. The UI stays quiet, dense and
predictable: the terminals are the content, and everything else gets out of their way. It is
also polished: a rough edge in something people look at all day is a bug, not a detail.

### Polish

- Everything lines up. Edges, baselines and icons sit on the same grid, and padding is the same
  wherever the same kind of thing appears.
- Nothing jumps. Layout doesn't shift when content loads, a hover appears or a label changes
  length. Space for a badge, a spinner or a status is reserved before it shows.
- No flashes. The theme is loaded before the first frame, so the window never shows a default
  color first. Switching themes, by hand or when the system appearance changes, repaints in one
  frame.
- Every state is designed: hover, pressed, focus, disabled, selected, empty, loading and error.
  Nothing falls back to an unstyled default.
- Motion is short and purposeful, and only explains a change (something opened, moved or
  appeared). Nothing animates for decoration.
- A visual change is checked in at least one dark and one light theme, and on both platforms,
  before it's done. The PR shows before and after.

### Themes

Visr's look is fully driven by themes. A theme is a TOML file that sets every color role, for the
chrome and for the terminals. Changing the theme never needs a rebuild.

- The user's `config.toml` names the active theme: `theme = "tokyo_night"`. Without that line,
  the theme follows the system: `default` when the system appearance is dark and `default_light`
  when it is light, and it switches live when the system changes. A theme the user names always
  wins and does not follow the system.
- `config` looks the name up in the user's themes folder (`<config dir>/themes/<name>.toml`)
  first, then in the built-in themes. A user file with a built-in's name replaces it.
- A theme file has a `[meta]` table (`name`, `appearance = "dark" | "light"`), a `[ui]` table of
  chrome roles and a `[terminal]` table with the 16 ANSI colors plus foreground, background,
  cursor and selection. Colors are `#rrggbb` or `#rrggbbaa`.
- A user theme may set only some roles. Every missing role is filled from the built-in theme of
  the same appearance, so a partial theme always renders. Unknown keys and invalid values are
  reported to the person once, by key and file, and the rest of the theme still applies. A
  theme that can't be parsed at all falls back to `default` with the same kind of message.
- The built-in themes live in `crates/theme/themes/` and are embedded in the binary. `default`
  is dark and has a light sibling, `default_light`. Both set every role: adding a role means
  adding it to every built-in theme in the same change. A built-in theme with a missing or
  unknown key is reported at startup like a user theme would be, so it shows up the first time
  the app runs.
- Adding a role is a decision: it is a new knob every theme author will see. Prefer an existing
  role. A new one gets a `///` comment saying where it is used, and a line in `CHANGELOG.md`,
  because it changes the theme format.
- The theme format is documented in `docs/themes.md`, with every role, where it shows, and a
  complete example file. A change to the format updates that page in the same commit.

### Tokens

- Colors come from the active theme, through `ui`. A screen in `app` never invents one.
- Everything else is a fixed token in `ui`, the same in every theme: spacing, radius, border
  width, icon size, font size, line height, shadow and duration. Themes change color, not
  layout, so alignment never depends on which theme is active.
- Spacing follows one scale on a 4 px base (`SPACE_1` = 4, `SPACE_2` = 8, `SPACE_3` = 12, …).
  Font sizes and radii have their own short scales. A value between two steps is a sign the
  layout is wrong, not that the scale needs a new step.
- Color roles are named for their purpose, not their hue: `text_muted`, `surface_raised`,
  `border_focused`, `status_blocked`, never `gray_400`. No component checks which theme or
  appearance is active.
- The chrome uses `[ui]` roles only, and the terminal grid uses `[terminal]` colors only. Neither
  borrows from the other, so a theme author can tune one without breaking the other.

### Components

- A component in `ui` is a `RenderOnce` element built from plain data and callbacks. It holds no
  app state, calls nothing outside `ui`, and lets the caller own any state that outlives a frame.
- One component per concept. Variants (primary, subtle, danger; small, medium) are an enum
  parameter, not a sibling component.
- Every interactive component draws all its states from tokens: default, hover, pressed, focused,
  disabled and, where it applies, selected. A state it can't show is a bug.

### Interaction

- Everything works from the keyboard. Focus is always visible, moves in reading order, and is
  never lost when something opens or closes: closing a menu, dialog or pane returns focus to where
  it came from.
- Hit targets are at least 24 px tall even when the drawn element is smaller.
- Anything that can be clicked shows the pointer it needs and reacts on hover within the same
  frame.
- Menus, tooltips and buttons that run an action show its configured shortcut, read from
  `config`, never written into the label.
- Destructive actions (killing a terminal, closing a tab with running programs) ask once, name
  what will be lost, and offer the safe option as the default.

### Status

- A terminal's status (working, blocked, done, exited) is shown by color **and** a second cue
  (icon, shape or label), so it reads without color vision and in either theme.
- Status that needs a person, such as blocked, is the most visible thing on screen. Everything
  else is quieter than it.
- Every view has its empty, loading and error states designed, not left blank. An error says what
  failed and what to do next, in one or two sentences.

### Text

- Sentence case everywhere: "Close tab", not "Close Tab". No trailing period on labels, buttons
  or menu items. Tooltips and messages that are full sentences end with one.
- Short, plain words that name the result: "Split right", not "Perform horizontal split".
- Long names (paths, branches, commands) truncate in the middle and show the full value in a
  tooltip.

### Platform

- Follow each platform's window conventions: traffic lights and title bar on macOS, caption
  buttons on Windows, and each platform's primary modifier (⌘ or Ctrl) in shortcuts.
- Respect the system's reduced-motion, contrast and font-scaling settings. Text sizes come from
  tokens that scale with the user's font size setting.

## Rust guidelines

### Types

- Let the type system rule out bugs. Prefer an enum over a `bool` or a string with special values,
  and `Option<T>` over sentinel values (`0`, `""`, `-1`).
- Don't let values of the same type sit next to each other where they can be swapped. More than a
  handful of arguments, or two neighbors of one type (two `bool`s, two `String`s), means a struct
  with named fields. No builders for internal structs.
- Keep fields private, and the narrowest visibility that compiles (`pub(super)`, `pub(crate)`).
- Derive `Debug`, `Clone`, `PartialEq` and `Eq` on data types where they make sense. Add `Default`
  only when there's a real default.

### Errors

- **NEVER** use `.unwrap()`. Propagate with `?`, handle the error, or use
  `.expect("…")` with a message saying why it can't fail. `Mutex::lock().unwrap()` and
  `Condvar::wait(…).unwrap()` are the accepted exceptions: a poisoned lock means another thread
  already panicked.
- Library crates return their own error enums. In `app`, an error that reaches a person becomes a
  full sentence that says what failed and, when it helps, what to do.
- Don't add `thiserror` or `anyhow` without asking (see Dependencies). A hand-written `Display`
  is a few lines.
- Behavior-preserving refactors keep the same errors, in the same order, with the same side
  effects.

### Functions

- One job per function. Return early, use let-else, and keep nesting shallow.
- Borrow (`&T`, `&str`, `&[T]`) unless the function has to own the value.
- Pull decisions out of render and platform code into pure functions, so they read on their own.
- Prefer iterators and adapters when they read clearer than a loop. A `for` loop that returns
  early, or collects errors in order, is fine.
- Extract a repeated subexpression into a named variable or helper.

### Memory and performance

- No avoidable allocation in hot paths (render, terminal output, key handling): prefer `&str` to
  `String`, use `Cow<'_, str>` when ownership is only sometimes needed, and `Vec::with_capacity`
  when the size is known.
- Clone explicitly and only when needed. Watch for clones hidden in closures and per-row maps.
- Cache what a frame would otherwise recompute, and invalidate it on the event that changes it.
- Parallelism comes from GPUI's executors and plain threads. Don't add `tokio` or `rayon`.

### Concurrency

- UI state lives in GPUI entities and changes only through `update`. Background work hands
  results back. It never shares mutable UI state.
- Use channels for message passing. Hold a `Mutex` only briefly, and never across a blocking call.

### Unsafe

- **NEVER** write `unsafe` without need, and every block gets a `// SAFETY:` comment stating the
  invariant. FFI declarations live inside the `cfg`'d function that uses them.

### Imports and dependencies

- Standard library first, then external crates, then local modules. Let `rustfmt` order them.
- No wildcard imports, except preludes, `use super::*` in a module's own submodules,
  and constant-only modules such as a theme.
- **No new dependency without asking.** Propose it first, with why std or 20 lines of our own
  won't do. Prefer what's already in the tree. Shell out to the user's own tools (`git`, `curl`,
  editors) rather than linking a library for them. Pin versions in the workspace `Cargo.toml`
  (`[workspace.dependencies]`), and let `Cargo.lock` follow through `cargo`, never by hand.
- **Git dependencies are pinned by `rev`.** `gpui` and anything else taken from a git repository
  names an exact commit, never a branch. Moving to a newer commit is its own `build:` commit that
  says why.
- Every dependency's license must be compatible with ours (see License).

## Documentation and comments

- All code, comments and docs are in English. **NEVER** use emoji in code, comments, commits or
  docs. UI glyphs (⌘, nerd font icons) are the exception.
- Every module starts with a `//!` line saying what it is. Every type, and every function whose
  behavior isn't obvious from its name, gets a `///` comment: one or two plain sentences on what
  it is for, or why it's done this way, and what a caller has to know.
- Comments explain _why_, never _what_ the next line does. Keep them true when the code changes.
- **NEVER** commit commented-out code, `dbg!`, or stray `println!`/`eprintln!`.
- Crates have no `examples/` directories.

## Security

- Processes and the network are untrusted. Anything downloaded to run, like the herdr-core
  installer, goes into a private folder, is written to a new file, and runs from that file,
  **NEVER** piped into a shell. https is required for herdr's own servers.
- Run a program by the exact name you mean. Never a generic name another tool could answer to.
- **NEVER** commit secrets, tokens or personal data, and never log terminal contents.
- Paths and names from the core, the filesystem or git are data. Pass them as arguments, never
  through a shell string.

## License

Visr is licensed under the GNU General Public License, version 3 or any later version (`LICENSE`).
Every crate declares `license = "GPL-3.0-or-later"` through `license.workspace = true`.

- Contributions are accepted under the same terms. `CONTRIBUTING.md` says so, and no other license
  header or notice is added to our files.
- A new dependency, font, icon pack or other bundled asset must be under a license compatible with
  GPL-3.0-or-later: MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0 or OFL-1.1 (fonts
  only). Bundled assets (fonts, icon packs, images) may also be under CC0-1.0, CC-BY-4.0, the
  Unlicense or Powerline's "Free License". Each asset keeps its license or notice file next to
  it, which must also carry the attribution CC-BY-4.0 requires.
  **NEVER** add GPL, LGPL, AGPL, MPL or unlicensed code without asking. Copying code from
  elsewhere counts too: keep its notice and note where it came from.

## Crate publishing

Visr ships as an app, not as libraries. Every crate sets `publish = false`, so nothing reaches
crates.io by accident. Changing that for a crate is a decision for the maintainer.

## Building and checks

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p visr            # needs herdr-core installed; Visr offers to install it if missing
```

These are the CI steps. Clippy stays clean with `--all-targets`, and **NEVER** silence it with
`#[allow]`. Fix the code instead: `too_many_arguments` means a struct. The one exception is a
platform-only `#[cfg_attr(…, allow(…))]` with a comment saying why.

Agents don't launch the app or any long-running process. Ask the person to run it, and list the
manual checks the change still needs (what to click, what to look for).

Environment: `VISR_CONFIG` points at another config file, and its folder's `themes/` is used
for user themes. `VISR_HERDR_RELEASES_URL` points the
installer at a mirror or a `file://` fixture, and the app warns when it is set.

## Code style

- 2-space indent and 100-column lines (`rustfmt.toml`). snake_case for functions and modules,
  PascalCase for types, SCREAMING_SNAKE_CASE for consts.
- Module order: `use` block, `const`s, types, impls, and private helpers last.
- In render, prefer `.when()`, `.children(option)` and `.map()` over branching.
- Edit files with an editor tool, not `sed`/`perl` one-liners on source.
- Split a file before it grows past ~1000 lines.

## Branches

- `main` is the principal branch and holds released code only. It receives PRs only from `dev`,
  and each one is a release.
- `dev` is the work branch and the repository's default. Every PR except a release PR targets
  `dev`, and `dev` never takes a PR from `main`. `## [Unreleased]` in `CHANGELOG.md` lists what is
  on `dev` but not yet on `main`.
- Work branches start from `dev` and are named `feat/…`, `fix/…`, `chore/…`, `docs/…` or `ci/…`.
- Nothing is pushed directly to `main` or `dev`. Everything goes through a PR.
- PRs into `dev` are squash-merged. Release PRs from `dev` into `main` use a merge commit.

## Commits

Agents commit only when the person they work for asks, and show the full message first. Agents
**NEVER** push, open PRs, merge or tag unless the person explicitly asks for it in the task.

Conventional Commits: `type(scope): description`, imperative, lowercase, no trailing period,
header ≤ 72 characters. Add a body (lines ≤ 100) only when the header doesn't make the reason
clear. Breaking changes get `!` and a `BREAKING CHANGE:` footer. The scope is the crate (`app`, `ui`, `theme`,
`app-state`, `term`, `config`, `hc-client`, `hc-proto`) or `release`. `ci:`, `build:` and `docs:`
take no scope. Each commit builds and passes the checks on its own, and one commit does one thing:
a refactor and a feature are two commits.

Whether a commit carries an AI attribution trailer (such as `Co-Authored-By`) is up to the
contributor who makes it.

## Pull requests

The PR title follows the same Conventional Commits rules as a commit header, because a squash merge
turns it into the commit on `dev`. The PR body follows `.github/pull_request_template.md`: Summary,
Changes, Breaking changes, and Screenshots / Additional context.

Summary is one or two sentences on why the change is needed and what a person gets out of it. The
bullets go under Changes, one per behavior change. They are lowercase, imperative and one line
each, and they describe behavior rather than files. Screenshots of a visual change go under the
screenshots section, with before and after in a dark and a light theme. State which platforms you
ran it on.

## Changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/). A change someone using Visr can notice adds an
entry to `## [Unreleased]` in the same commit, under `Added`, `Changed`, `Deprecated`, `Removed`,
`Fixed` or `Security`.

- One short line per entry, saying what a person can now do or what no longer goes wrong, in
  their words: "Split panes open in the focused terminal's folder", not "pass working_dir to
  start".
- Leave out refactors, CI and anything else nobody using the app can observe.
- Newest version first. Each version heading links to its diff at the bottom of the file.

## Releases

1. Open a PR into `dev` with the commit `chore(release): bump visr to X.Y.Z`. Bump
   `[workspace.package].version` in the root `Cargo.toml` and refresh the lockfile with
   `cargo update --workspace --offline`. Only the `version` lines of the workspace crates in
   `Cargo.lock` may change.
   In `CHANGELOG.md`, rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`, open a new empty
   `## [Unreleased]` above it, and update the links.
   While on 0.x, a `feat` since the last release bumps the minor version, and fixes alone bump the
   patch.
2. Open a release PR from `dev` into `main` and merge it with a merge commit.
3. A maintainer tags the merge commit on `main` and pushes the tag:
   `git tag -a vX.Y.Z -m "visr X.Y.Z" && git push origin vX.Y.Z`. The tag push runs the release
   workflow, which builds the universal macOS dmg and the Windows x64 and arm64 executables and
   publishes the GitHub release with that version's changelog section. It can be dry-run from the
   Actions tab.

Signing: the macOS app is ad-hoc signed (`codesign --sign -`) and not notarized, and the Windows
executables are unsigned. The README tells people how to open a build the first time on each
platform. Adding real signing later only changes the release workflow.

## Before you say it's done

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes, with no new `#[allow]`
- [ ] Platform-specific code leaves nothing unused on the other platforms
- [ ] No `.unwrap()` (lock and condvar poisoning aside), no `unsafe` without
      `// SAFETY:`
- [ ] Every new module opens with a `//!` line, and new types and non-obvious functions have `///`
- [ ] No hardcoded colors, no duplicated alignment literals, no styling outside `ui`'s tokens
- [ ] New or changed components show every state and work from the keyboard
- [ ] Visual changes were checked in a dark and a light theme; theme format changes update every
      built-in theme and `docs/themes.md`
- [ ] No commented-out code, `dbg!`, stray prints, emoji or secrets
- [ ] No new dependency that wasn't agreed on
- [ ] A change a person can notice has its line under `## [Unreleased]` in `CHANGELOG.md`
- [ ] The manual checks the change still needs are listed for the person running the app

**Remember:** clarity and maintainability over cleverness, and the smallest change that does the
job.
