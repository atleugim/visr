//! The window chrome's layout tokens: sizes, gaps, radii and offsets, in logical pixels.

/// Height of the strip the macOS traffic lights sit in, above the sidebar.
pub const TITLEBAR_HEIGHT: f32 = 40.0;
/// The height of a row in the sidebar.
pub const ROW_HEIGHT: f32 = 32.0;
/// The box a sidebar row's or the toolbar's icon is centered in.
pub const ICON_BOX: f32 = 18.0;
/// Room the macOS traffic lights take at the window's top left.
const TRAFFIC_LIGHTS_WIDTH: f32 = 72.0;
/// The title bar's buttons are squares this big.
pub const TITLEBAR_BUTTON: f32 = 24.0;
/// The gap between the window's edges and the panes, the same on every side and in every state.
pub const WINDOW_GAP: f32 = 8.0;
/// How far down the middle of a collapsed top left pane header is: what the traffic lights, the
/// sidebar toggle and the sidebar's + all center on.
const TITLE_CENTER: f32 = WINDOW_GAP + PANE_BORDER_WIDTH + PANE_HEADER_HEIGHT / 2.0;
/// Where the traffic lights' left edge is, in every window that has them.
pub const TRAFFIC_LIGHTS_LEFT: f32 = 14.0;
/// How tall the macOS traffic lights are.
const TRAFFIC_LIGHTS_HEIGHT: f32 = 14.0;
/// Where the traffic lights' top is, so that their middle is the title's.
pub const TRAFFIC_LIGHTS_TOP: f32 = TITLE_CENTER - TRAFFIC_LIGHTS_HEIGHT / 2.0;
/// Pushes the sidebar strip's content down until it is centered on the title's middle.
pub const TITLEBAR_NUDGE: f32 = 2.0 * TITLE_CENTER - TITLEBAR_HEIGHT;
/// Where a pane's card starts, with the sidebar gone: the main area's margin, then its border.
pub const PANE_EDGE: f32 = WINDOW_GAP + PANE_BORDER_WIDTH;
/// Where the sidebar toggle is fixed: past the traffic lights on macOS, else a little inside the
/// pane's edge.
pub const TOGGLE_LEFT: f32 = PANE_EDGE
  + if cfg!(target_os = "macos") {
    TRAFFIC_LIGHTS_WIDTH
  } else {
    4.0
  };
/// Where the sidebar toggle's top is, so that its middle is the title's.
pub const TOGGLE_TOP: f32 = TITLE_CENTER - TITLEBAR_BUTTON / 2.0;
/// The gap between the fixed toggle and what follows it.
pub const TOGGLE_GAP: f32 = 8.0;
/// The corner radius of a pane's card.
pub const PANE_RADIUS: f32 = 10.0;
/// The height of a pane's header.
pub const PANE_HEADER_HEIGHT: f32 = 30.0;
/// A pane header's left padding when nothing in the corner needs room.
pub const HEADER_PADDING: f32 = 12.0;
/// The gap between panes, which is also the handle that resizes them.
pub const DIVIDER_WIDTH: f32 = 8.0;
/// The width of a pane's border.
pub const PANE_BORDER_WIDTH: f32 = 1.0;
/// The padding between a pane's left border and its terminal.
pub const PANE_PADDING_LEFT: f32 = 8.0;
/// The padding between a pane's terminal and its bottom border.
pub const PANE_PADDING_BOTTOM: f32 = 4.0;
/// What a pane's card takes around its terminal, from the measures above: a new pane's size is
/// worked out with these, so the card must be styled from the same consts.
pub const PANE_CHROME_WIDTH: f32 = 2.0 * PANE_BORDER_WIDTH + PANE_PADDING_LEFT;
/// What a pane's card takes above and below its terminal, as the width above does on its sides.
pub const PANE_CHROME_HEIGHT: f32 =
  PANE_HEADER_HEIGHT + PANE_PADDING_BOTTOM + 2.0 * PANE_BORDER_WIDTH;
/// Room before a panel row's content; its highlight reaches both edges of the panel.
pub const ROW_INSET: f32 = 12.0;
/// The smallest height a clickable element may have, so it is easy to hit.
pub const MIN_HIT_HEIGHT: f32 = 24.0;
