// SPDX-License-Identifier: GPL-3.0-only

//! Pure (Wayland-free) state, MRU ordering, grid layout, and key handling for
//! the isolated Alt+Tab / Super+Tab window switcher.

use std::time::{Duration, Instant};

use cosmic::iced::keyboard::{
    Modifiers,
    key::{Key, Named},
};

/// How long Alt/Super must be held before the switcher panel is painted.
/// A quick tap switches immediately without flashing the UI.
pub const PANEL_DELAY_MS: u64 = 150;

/// When to check if the modifier was ever seen held after opening.
/// If the user released Alt/Super before the layer surface gained keyboard
/// focus, no release event will arrive, so this timer confirms the switch.
pub const HELD_CHECK_MS: u64 = 400;

/// Two cycle events from different sources (D-Bus shortcut + Wayland keyboard)
/// closer than this are treated as the same key press.
pub const CYCLE_DEDUP: Duration = Duration::from_millis(80);

/// Smallest preview height before the grid starts scrolling.
pub const MIN_PREVIEW_H: f32 = 160.0;
/// Largest preview height so a few windows stay sleek and well-proportioned.
pub const MAX_PREVIEW_H: f32 = 240.0;
/// 16:10 preview box aspect ratio.
pub const PREVIEW_ASPECT: f32 = 1.6;
/// Space between cards in the grid.
pub const GRID_GAP: f32 = 14.0;
/// Height reserved inside each card for the app icon + title row.
pub const LABEL_H: f32 = 34.0;
/// Padding around the switcher panel card.
pub const PANEL_PADDING: f32 = 22.0;
/// Height reserved for the switcher header bar.
pub const HEADER_H: f32 = 34.0;

/// Most-recently-used ordering of window identifiers. Front = most recent.
#[derive(Debug, Clone)]
pub struct Mru<K: Eq + Clone> {
    order: Vec<K>,
}

impl<K: Eq + Clone> Default for Mru<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Clone> Mru<K> {
    pub fn new() -> Self {
        Self { order: Vec::new() }
    }

    /// Move `key` to the front, inserting it if unknown.
    pub fn touch(&mut self, key: K) {
        self.order.retain(|k| k != &key);
        self.order.insert(0, key);
    }

    /// Drop a closed window from MRU history.
    pub fn remove(&mut self, key: &K) {
        self.order.retain(|k| k != key);
    }

    /// Retain only keys still present in `present`.
    pub fn retain_present(&mut self, present: &[K]) {
        self.order.retain(|k| present.contains(k));
    }

    /// Keys of `present` in MRU order, followed by any unseen keys in order.
    pub fn ordered(&self, present: &[K]) -> Vec<K> {
        let known = self.order.iter().filter(|k| present.contains(k));
        let unknown = present.iter().filter(|k| !self.order.contains(k));
        known.chain(unknown).cloned().collect()
    }
}

/// Result of releasing the switcher modifier or confirming the selection.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome<K> {
    Activate(K),
    Nothing,
}

/// State machine of one switcher interaction, from open to confirm/cancel.
#[derive(Debug, Clone)]
pub struct SwitcherState<K: Eq + Clone> {
    order: Vec<K>,
    index: usize,
    panel_visible: bool,
    sticky: bool,
    modifier_seen: bool,
}

impl<K: Eq + Clone> SwitcherState<K> {
    /// Opens a new switcher session.
    ///
    /// - `order` is most-recent-first.
    /// - When `sticky` is false (Alt+Tab / Super+Tab): starts hidden for
    ///   `PANEL_DELAY_MS` and preselects index 1 (or last if `reverse`).
    /// - When `sticky` is true (overview mode): starts visible immediately,
    ///   stays open without holding a modifier, and preselects index 0.
    pub fn open(order: Vec<K>, reverse: bool, sticky: bool) -> Self {
        let index = if sticky {
            0
        } else {
            match order.len() {
                0 | 1 => 0,
                len if reverse => len - 1,
                _ => 1,
            }
        };
        Self {
            order,
            index,
            panel_visible: sticky,
            sticky,
            modifier_seen: sticky,
        }
    }

    /// Move selection by `delta`, wrapping around.
    pub fn cycle(&mut self, delta: i32) {
        let len = self.order.len() as i64;
        if len == 0 {
            return;
        }
        self.index = (self.index as i64 + i64::from(delta)).rem_euclid(len) as usize;
    }

    pub fn selected(&self) -> Option<&K> {
        self.order.get(self.index)
    }

    pub fn selected_index(&self) -> usize {
        self.index
    }

    pub fn order(&self) -> &[K] {
        &self.order
    }

    pub fn show_panel(&mut self) {
        self.panel_visible = true;
    }

    pub fn panel_visible(&self) -> bool {
        self.panel_visible
    }

    pub fn is_sticky(&self) -> bool {
        self.sticky
    }

    /// Record that Alt or Super was observed held during this session.
    pub fn note_modifier_held(&mut self) {
        self.modifier_seen = true;
    }

    /// True if the modifier was never observed held (released before focus).
    pub fn modifier_already_released(&self) -> bool {
        !self.sticky && !self.modifier_seen
    }

    /// Remove a closed window while keeping the selection on a valid neighbour.
    pub fn remove(&mut self, key: &K) {
        let Some(pos) = self.order.iter().position(|k| k == key) else {
            return;
        };
        self.order.remove(pos);
        if pos < self.index {
            self.index -= 1;
        }
        self.index = self.index.min(self.order.len().saturating_sub(1));
    }

    /// Confirm the current selection.
    ///
    /// In non-sticky mode with fewer than 2 windows, there is nothing to
    /// switch to (unless the user explicitly confirms in sticky mode).
    pub fn release(self) -> Outcome<K> {
        if self.order.is_empty() || (!self.sticky && self.order.len() < 2) {
            return Outcome::Nothing;
        }
        match self.order.into_iter().nth(self.index) {
            Some(key) => Outcome::Activate(key),
            None => Outcome::Nothing,
        }
    }
}

/// Where a selection step came from, to deduplicate compositor shortcut +
/// keyboard focus delivering the same Tab press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleSource {
    Dbus,
    Keyboard,
}

/// Whether a cycle from `source` at `now` is a duplicate of `last`.
pub fn should_dedup(
    last: Option<(Instant, CycleSource)>,
    now: Instant,
    source: CycleSource,
) -> bool {
    matches!(last, Some((at, prev)) if prev != source && now.duration_since(at) < CYCLE_DEDUP)
}

/// Uniform grid metrics for window cards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    pub cols: usize,
    pub preview_w: f32,
    pub preview_h: f32,
    pub scrolls: bool,
}

/// Computes an optimal uniform grid for `count` previews inside `avail_w` × `avail_h`.
pub fn layout(avail_w: f32, avail_h: f32, count: usize, gap: f32, label_h: f32) -> GridSpec {
    let n = count.max(1);
    let avail_w = avail_w.max(MIN_PREVIEW_H * PREVIEW_ASPECT);
    let avail_h = avail_h.max(MIN_PREVIEW_H + label_h);
    // Cap columns so a single row stays visually balanced (max 5 cols on wide screens).
    let max_cols = n.min(5);
    let (cols, height) = (1..=max_cols)
        .map(|cols| {
            let rows = n.div_ceil(cols);
            let cell_w = (avail_w - gap * (cols - 1) as f32) / cols as f32;
            let by_width = cell_w / PREVIEW_ASPECT;
            let by_height = (avail_h - gap * (rows - 1) as f32) / rows as f32 - label_h;
            (cols, by_width.min(by_height).min(MAX_PREVIEW_H))
        })
        .fold((1, f32::MIN), |best, cur| if cur.1 >= best.1 { cur } else { best });

    if height >= MIN_PREVIEW_H {
        return GridSpec {
            cols,
            preview_w: (height * PREVIEW_ASPECT).round(),
            preview_h: height.round(),
            scrolls: false,
        };
    }

    let preview_w = (MIN_PREVIEW_H * PREVIEW_ASPECT).round();
    let fit = ((avail_w + gap) / (preview_w + gap)).floor() as usize;
    GridSpec {
        cols: fit.clamp(1, n),
        preview_w,
        preview_h: MIN_PREVIEW_H,
        scrolls: true,
    }
}

/// What a keyboard press means while the switcher is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Cycle(i32),
    Row(i32),
    Confirm,
    Cancel,
}

/// Map a key press to a switcher action. `None` means ignore.
pub fn key_action(key: &Key, modifiers: Modifiers) -> Option<KeyAction> {
    match key {
        Key::Named(Named::Escape) => Some(KeyAction::Cancel),
        Key::Named(Named::Tab) if modifiers.shift() => Some(KeyAction::Cycle(-1)),
        Key::Named(Named::Tab) => Some(KeyAction::Cycle(1)),
        Key::Named(Named::ArrowLeft) => Some(KeyAction::Cycle(-1)),
        Key::Named(Named::ArrowRight) => Some(KeyAction::Cycle(1)),
        Key::Named(Named::ArrowUp) => Some(KeyAction::Row(-1)),
        Key::Named(Named::ArrowDown) => Some(KeyAction::Row(1)),
        Key::Named(Named::Enter) => Some(KeyAction::Confirm),
        _ => None,
    }
}

/// Finds an output by exact connector name (e.g. `DP-3`). Empty name -> `None`.
pub fn output_named<O: Clone>(name: &str, outputs: &[(String, O)]) -> Option<O> {
    if name.is_empty() {
        return None;
    }
    outputs
        .iter()
        .find(|(output_name, _)| output_name == name)
        .map(|(_, output)| output.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mru_orders_most_recent_first_and_appends_unknowns() {
        let mut mru = Mru::new();
        mru.touch("a");
        mru.touch("b");
        mru.touch("a");
        assert_eq!(mru.ordered(&["x", "b", "a", "y"]), vec!["a", "b", "x", "y"]);
        mru.remove(&"b");
        assert_eq!(mru.ordered(&["a", "b"]), vec!["a", "b"]);
    }

    #[test]
    fn open_selects_previous_or_oldest_for_reverse() {
        let fwd = SwitcherState::open(vec!['a', 'b', 'c'], false, false);
        assert_eq!(fwd.selected(), Some(&'b'));
        assert!(!fwd.panel_visible());
        assert!(fwd.modifier_already_released());

        let rev = SwitcherState::open(vec!['a', 'b', 'c'], true, false);
        assert_eq!(rev.selected(), Some(&'c'));

        let sticky = SwitcherState::open(vec!['a', 'b'], false, true);
        assert_eq!(sticky.selected(), Some(&'a'));
        assert!(sticky.panel_visible());
        assert!(!sticky.modifier_already_released());
    }

    #[test]
    fn cycle_wraps_and_remove_preserves_valid_selection() {
        let mut st = SwitcherState::open(vec!['a', 'b', 'c', 'd'], false, false);
        assert_eq!(st.selected(), Some(&'b'));
        st.cycle(1);
        assert_eq!(st.selected(), Some(&'c'));
        st.cycle(2);
        assert_eq!(st.selected(), Some(&'a'));
        st.cycle(-1);
        assert_eq!(st.selected(), Some(&'d'));

        st.remove(&'a');
        assert_eq!(st.selected(), Some(&'d'));
        st.remove(&'d');
        assert_eq!(st.selected(), Some(&'c'));
    }

    #[test]
    fn release_handles_single_and_empty_windows() {
        assert_eq!(
            SwitcherState::open(Vec::<char>::new(), false, false).release(),
            Outcome::Nothing
        );
        assert_eq!(
            SwitcherState::open(vec!['a'], false, false).release(),
            Outcome::Nothing
        );
        assert_eq!(
            SwitcherState::open(vec!['a'], false, true).release(),
            Outcome::Activate('a')
        );
        assert_eq!(
            SwitcherState::open(vec!['a', 'b'], false, false).release(),
            Outcome::Activate('b')
        );
    }

    #[test]
    fn key_actions_cover_tab_shift_tab_arrows_enter_and_escape() {
        assert_eq!(
            key_action(&Key::Named(Named::Tab), Modifiers::ALT),
            Some(KeyAction::Cycle(1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::Tab), Modifiers::ALT | Modifiers::SHIFT),
            Some(KeyAction::Cycle(-1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::Tab), Modifiers::LOGO),
            Some(KeyAction::Cycle(1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::ArrowLeft), Modifiers::empty()),
            Some(KeyAction::Cycle(-1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::ArrowRight), Modifiers::empty()),
            Some(KeyAction::Cycle(1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::ArrowUp), Modifiers::empty()),
            Some(KeyAction::Row(-1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::ArrowDown), Modifiers::empty()),
            Some(KeyAction::Row(1))
        );
        assert_eq!(
            key_action(&Key::Named(Named::Enter), Modifiers::empty()),
            Some(KeyAction::Confirm)
        );
        assert_eq!(
            key_action(&Key::Named(Named::Escape), Modifiers::empty()),
            Some(KeyAction::Cancel)
        );
        assert_eq!(
            key_action(&Key::Character("a".into()), Modifiers::empty()),
            None
        );
    }

    #[test]
    fn dedup_drops_cross_source_within_window_only() {
        let t0 = Instant::now();
        let t_near = t0 + Duration::from_millis(30);
        let t_far = t0 + Duration::from_millis(120);
        assert!(should_dedup(
            Some((t0, CycleSource::Dbus)),
            t_near,
            CycleSource::Keyboard
        ));
        assert!(!should_dedup(
            Some((t0, CycleSource::Dbus)),
            t_near,
            CycleSource::Dbus
        ));
        assert!(!should_dedup(
            Some((t0, CycleSource::Dbus)),
            t_far,
            CycleSource::Keyboard
        ));
    }

    #[test]
    fn grid_layout_handles_zero_few_and_many_windows() {
        let g0 = layout(1600.0, 900.0, 0, GRID_GAP, LABEL_H);
        assert!(g0.cols >= 1);

        let g3 = layout(1600.0, 900.0, 3, GRID_GAP, LABEL_H);
        assert_eq!(g3.cols, 3);
        assert!(!g3.scrolls);
        assert_eq!(g3.preview_h, MAX_PREVIEW_H);

        let g40 = layout(1400.0, 760.0, 40, GRID_GAP, LABEL_H);
        assert!(g40.scrolls);
        assert_eq!(g40.preview_h, MIN_PREVIEW_H);
    }

    #[test]
    fn output_named_matches_exact_non_empty_connector() {
        let outputs = vec![("DP-3".to_string(), 1), ("HDMI-A-1".to_string(), 2)];
        assert_eq!(output_named("DP-3", &outputs), Some(1));
        assert_eq!(output_named("HDMI-A-1", &outputs), Some(2));
        assert_eq!(output_named("", &outputs), None);
        assert_eq!(output_named("dp-3", &outputs), None);
    }
}
