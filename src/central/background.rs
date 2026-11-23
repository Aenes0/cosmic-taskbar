// SPDX-License-Identifier: GPL-3.0-only

//! The background apps (StatusNotifierItem) in the Central: the same items
//! and the same list of the ones on the bar (`TrayConfig::visible`) the tray
//! applet keeps, but nothing here opens a popup: the apps' icons are a strip
//! of the panel (and, the chosen ones, on the bar), and an app's menu is a
//! page of the panel.

use std::collections::BTreeMap;

use cosmic::{
    Task, app,
    applet::token::subscription::{TokenRequest, TokenUpdate, activation_token_subscription},
    cctk::sctk::reexports::calloop,
    cosmic_config::{self, CosmicConfigEntry},
    iced::Subscription,
    widget::icon,
};

use crate::central::{
    app::Message,
    tray::{
        components::status_menu,
        config::{CONFIG_ID, TrayConfig},
        subscriptions::{
            status_notifier_item::{StatusNotifierItem, StatusNotifierItemProxy},
            status_notifier_watcher,
        },
    },
};

#[derive(Clone, Debug)]
pub enum Msg {
    /// An app came or went.
    Watcher(status_notifier_watcher::Event),
    /// What an app's menu says or asks, by the app's id here.
    Menu((usize, status_menu::Msg)),
    /// An app's Id and Title, read from it.
    Identity(usize, String, String),
    /// The list of those on the bar changed (maybe in another process).
    Config(TrayConfig),
    Token(TokenUpdate),
    /// A press on its icon on the bar: the same as `Activate`, and the
    /// bar's own press, which would open the panel, goes without.
    BarPress(usize),
    /// A press on the `^` arrow on the bar: opens or closes the dedicated
    /// background apps page, without triggering the bar's own press.
    BarArrow,
    /// Opens the dedicated background apps page directly in edit mode.
    OpenEdit,
    /// Toggles edit mode on the dedicated background apps page.
    ToggleEdit,
    /// What the app's icon does: its own action, or its menu where it has
    /// nothing else.
    Activate(usize),
    /// The app's menu, as a page of the panel.
    OpenMenu(usize),
    /// Puts the app on the bar, or takes it off.
    Toggle(usize),
    /// Moves the app one step earlier (`true`) or later (`false`) in the order.
    Move(usize, bool),
}

#[derive(Default)]
pub struct Background {
    items: BTreeMap<usize, status_menu::State>,
    last_id: usize,
    config: TrayConfig,
    config_handle: Option<cosmic_config::Config>,
    token_tx: Option<calloop::channel::Sender<TokenRequest>>,
    /// The app whose menu the panel's page shows.
    pub open: Option<usize>,
}

impl Background {
    /// With the list of the apps on the bar as it's saved.
    pub fn load() -> Self {
        let (config_handle, config) = match cosmic_config::Config::new(CONFIG_ID, TrayConfig::VERSION) {
            Ok(handle) => {
                let config = TrayConfig::get_entry(&handle).unwrap_or_else(|(_, config)| config);
                (Some(handle), config)
            }
            Err(why) => {
                tracing::error!(why = why.to_string(), "Error opening the tray's config");
                (None, TrayConfig::default())
            }
        };
        Self { config, config_handle, ..Self::default() }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every app, in display order, by its id here.
    pub fn apps(&self) -> impl Iterator<Item = (usize, &status_menu::State)> {
        let present: Vec<(usize, Option<&str>)> = self
            .items
            .iter()
            .map(|(id, state)| (*id, state.identity.key()))
            .collect();
        let items = &self.items;
        ordered_indices(&present, &self.config.visible)
            .into_iter()
            .filter_map(move |id| Some((id, items.get(&id)?)))
    }

    pub fn menu_state(&self, id: usize) -> Option<&status_menu::State> {
        self.items.get(&id)
    }

    /// What lasts of an app between runs: its Id, or else its Title.
    pub fn key_of(&self, id: usize) -> Option<&str> {
        self.items.get(&id)?.identity.key()
    }

    pub fn id_of(&self, key: &str) -> Option<usize> {
        self.items
            .iter()
            .find(|(_, state)| state.identity.key() == Some(key))
            .map(|(id, _)| *id)
    }

    /// Whether clicking the app should open its menu (true when `ItemIsMenu`
    /// is set or the app exports a DBusMenu, as libappindicator/Electron and
    /// many Qt tray apps treat `Activate(0, 0)` as a no-op).
    pub fn is_menu(&self, id: usize) -> Option<bool> {
        let item = &self.items.get(&id)?.item;
        Some(item.is_menu() || item.menu_proxy().is_some())
    }

    /// Whether the app shows on the bar: new apps show by default unless explicitly hidden.
    pub fn is_visible(&self, id: usize) -> bool {
        self.key_of(id)
            .is_some_and(|key| is_key_visible(&self.config.hidden, key))
    }

    /// The apps shown on the bar right now, in display order: id, icon and name.
    pub fn on_bar(&self) -> Vec<(usize, icon::Handle, String)> {
        self.apps()
            .filter_map(|(id, state)| {
                let key = state.identity.key()?;
                if !is_key_visible(&self.config.hidden, key) {
                    return None;
                }
                let name = state.identity.label().unwrap_or(key).to_owned();
                Some((id, state.icon_handle().clone(), name))
            })
            .collect()
    }

    /// An app on the bar, or off it.
    pub fn toggle(&mut self, id: usize) {
        let Some(key) = self.key_of(id).map(str::to_owned) else {
            return;
        };
        let hidden = toggle_visible(&self.config.hidden, &key);
        self.set_hidden(hidden);
    }

    /// Moves an app up (`up == true`) or down (`up == false`) in the display order.
    pub fn move_item(&mut self, id: usize, up: bool) {
        let Some(key) = self.key_of(id).map(str::to_owned) else {
            return;
        };
        let present = self.present_keys_ordered();
        let refs: Vec<&str> = present.iter().map(String::as_str).collect();
        let visible = move_visible(&self.config.visible, &refs, &key, up);
        self.set_visible(visible);
    }

    /// Moves `from_id` to the position of `to_id` in the display order
    /// (for drag-and-drop reordering).
    pub fn move_to(&mut self, from_id: usize, to_id: usize) {
        if from_id == to_id {
            return;
        }
        let (Some(from_key), Some(to_key)) = (
            self.key_of(from_id).map(str::to_owned),
            self.key_of(to_id).map(str::to_owned),
        ) else {
            return;
        };
        let present = self.present_keys_ordered();
        let refs: Vec<&str> = present.iter().map(String::as_str).collect();
        let visible = move_key_to(&self.config.visible, &refs, &from_key, &to_key);
        self.set_visible(visible);
    }

    fn present_keys_ordered(&self) -> Vec<String> {
        let mut keys = Vec::new();
        for (_, state) in self.apps() {
            if let Some(key) = state.identity.key()
                && !keys.iter().any(|existing: &String| existing == key)
            {
                keys.push(key.to_owned());
            }
        }
        keys
    }

    /// Writes the order list: the other processes, the bar's too, get it from the
    /// config's watch.
    fn set_visible(&mut self, visible: Vec<String>) {
        if self.config.visible == visible {
            return;
        }
        match self.config_handle.as_ref() {
            Some(handle) => {
                if let Err(why) = self.config.set_visible(handle, visible) {
                    tracing::error!(why = why.to_string(), "Failed to write the visible list");
                }
            }
            None => self.config.visible = visible,
        }
    }

    /// Writes the hidden list: the other processes, the bar's too, get it from the
    /// config's watch.
    fn set_hidden(&mut self, hidden: Vec<String>) {
        if self.config.hidden == hidden {
            return;
        }
        match self.config_handle.as_ref() {
            Some(handle) => {
                if let Err(why) = self.config.set_hidden(handle, hidden) {
                    tracing::error!(why = why.to_string(), "Failed to write the hidden list");
                }
            }
            None => self.config.hidden = hidden,
        }
    }

    pub fn set_config(&mut self, config: TrayConfig) {
        self.config = config;
    }

    pub fn learn(&mut self, id: usize, sni_id: &str, title: &str) {
        if let Some(state) = self.items.get_mut(&id) {
            state.identity.learn(sni_id, title);
        }
    }

    /// The app's menu shows: it's told, over D-Bus, as the tray tells it.
    pub fn open_menu(&mut self, id: usize) {
        if let Some(previous) = self.open.replace(id)
            && previous != id
            && let Some(state) = self.items.get(&previous)
        {
            state.closed();
        }
        if let Some(state) = self.items.get(&id) {
            state.opened();
        }
    }

    /// The menu's page is left.
    pub fn close(&mut self) {
        if let Some(id) = self.open.take()
            && let Some(state) = self.items.get(&id)
        {
            state.closed();
        }
    }

    /// An app came, went, or the watcher failed.
    pub fn event(&mut self, event: status_notifier_watcher::Event) -> app::Task<Message> {
        use status_notifier_watcher::Event;
        match event {
            Event::Connected(_) => Task::none(),
            Event::Registered(item) => {
                let (state, task) = status_menu::State::new(item);
                let proxy = state.item.item_proxy().clone();
                let existing = self
                    .items
                    .iter()
                    .find(|(_, previous)| previous.name() == state.name())
                    .map(|(id, _)| *id);
                // A name that registers again gets a fresh state, its
                // identity unknown, and read again.
                let id = existing.unwrap_or_else(|| {
                    self.last_id += 1;
                    self.last_id
                });
                self.items.insert(id, state);
                Task::batch([
                    read_identity(id, proxy),
                    task.map(move |message| cosmic::action::app(Message::Tray(Msg::Menu((id, message))))),
                ])
            }
            Event::Unregistered(name) => {
                let gone = self
                    .items
                    .iter()
                    .find(|(_, state)| state.name() == name)
                    .map(|(id, _)| *id);
                if let Some(id) = gone {
                    self.items.remove(&id);
                    if self.open == Some(id) {
                        self.open = None;
                    }
                }
                Task::none()
            }
            Event::Error(err) => {
                tracing::error!("Status notifier error: {err}");
                Task::none()
            }
        }
    }

    /// What an app's menu says: a click on an item of it, its layout.
    pub fn menu(&mut self, id: usize, message: status_menu::Msg) -> app::Task<Message> {
        match self.items.get_mut(&id) {
            Some(state) => state
                .update(message, id, self.token_tx.as_ref())
                .map(move |message| cosmic::action::app(Message::Tray(Msg::Menu((id, message))))),
            None => Task::none(),
        }
    }

    /// The app's own action, invoked directly over D-Bus (never stalled on
    /// an xdg_activation token that cosmic-panel's nested compositor may not
    /// deliver to embedded applets).
    pub fn activate(&self, id: usize) -> app::Task<Message> {
        let Some(state) = self.items.get(&id) else {
            return Task::none();
        };
        activate(id, &state.item, None)
    }

    /// The tokens asked for: to activate an app, or to click in its menu.
    pub fn token(&mut self, update: TokenUpdate) -> app::Task<Message> {
        match update {
            TokenUpdate::Init(tx) => {
                self.token_tx = Some(tx);
                Task::none()
            }
            TokenUpdate::Finished => {
                self.token_tx = None;
                Task::none()
            }
            TokenUpdate::ActivationToken { token, exec } => {
                let (Ok(id), Some(token)) = (exec.parse::<usize>(), token) else {
                    return Task::none();
                };
                match self.items.get_mut(&id) {
                    Some(state) => state
                        .update(status_menu::Msg::ClickToken(token), id, self.token_tx.as_ref())
                        .map(move |message| cosmic::action::app(Message::Tray(Msg::Menu((id, message))))),
                    None => Task::none(),
                }
            }
        }
    }

    /// What stays connected: the watcher, each app's icon and, for the one
    /// whose menu shows, its layout; the tokens; the list on the bar.
    pub fn subscription(&self, core: &app::Core, menu_shown: Option<usize>) -> Subscription<Msg> {
        let mut subscriptions = vec![
            status_notifier_watcher::subscription().map(Msg::Watcher),
            activation_token_subscription(0).map(Msg::Token),
        ];
        for (id, state) in &self.items {
            subscriptions.push(state.subscription(menu_shown == Some(*id)).with(*id).map(Msg::Menu));
        }
        subscriptions.push(core.watch_config::<TrayConfig>(CONFIG_ID).map(|update| {
            for why in update.errors.into_iter().filter(cosmic_config::Error::is_err) {
                tracing::error!(why = why.to_string(), "Error watching the tray's config");
            }
            Msg::Config(update.config)
        }));
        Subscription::batch(subscriptions)
    }
}

/// Reads Id and Title once, together, so a Title cannot fix the key before
/// the Id.
fn read_identity(id: usize, proxy: StatusNotifierItemProxy<'static>) -> app::Task<Message> {
    Task::future(async move {
        let sni_id = proxy.id().await.unwrap_or_else(|why| {
            tracing::warn!("Could not read Id of item {id}: {why}");
            String::new()
        });
        let title = proxy.title().await.unwrap_or_else(|why| {
            tracing::warn!("Could not read Title of item {id}: {why}");
            String::new()
        });
        cosmic::action::app(Message::Tray(Msg::Identity(id, sni_id, title)))
    })
}

/// The app's own action; if it fails, its menu.
fn activate(id: usize, item: &StatusNotifierItem, token: Option<String>) -> app::Task<Message> {
    let proxy = item.item_proxy().clone();
    Task::future(async move {
        if let Some(token) = token
            && let Err(why) = proxy.provide_xdg_activation_token(token).await
        {
            tracing::error!("Failed to give the token to {id}: {why}");
        }
        match proxy.activate(0, 0).await {
            Ok(()) => cosmic::action::app(Message::None),
            Err(why) => {
                tracing::error!("Activate failed for {id}: {why}");
                cosmic::action::app(Message::Tray(Msg::OpenMenu(id)))
            }
        }
    })
}

/// Whether an app with `key` shows on the bar (true by default unless hidden).
pub fn is_key_visible(hidden: &[String], key: &str) -> bool {
    !hidden.iter().any(|k| k == key)
}

/// Toggles whether `key` is visible on the bar by updating the `hidden` list.
/// New keys not in `hidden` are visible by default, so toggling hides them.
pub fn toggle_visible(hidden: &[String], key: &str) -> Vec<String> {
    let mut out = hidden.to_vec();
    if out.iter().any(|k| k == key) {
        out.retain(|k| k != key);
    } else {
        out.push(key.to_owned());
    }
    out
}

/// Orders present items: those in `order` first in that order, then the rest
/// in their original order.
pub fn ordered_indices(items: &[(usize, Option<&str>)], order: &[String]) -> Vec<usize> {
    let mut ordered: Vec<(usize, usize)> = Vec::new();
    let mut rest: Vec<usize> = Vec::new();
    for &(id, key) in items {
        match key.and_then(|key| order.iter().position(|o| o == key)) {
            Some(pos) => ordered.push((pos, id)),
            None => rest.push(id),
        }
    }
    ordered.sort_by_key(|&(pos, _)| pos);
    ordered.into_iter().map(|(_, id)| id).chain(rest).collect()
}

/// Moves `key` one step earlier (`up == true`) or later (`up == false`) among
/// the currently `present` keys, returning the updated persisted order list.
pub fn move_visible(order: &[String], present: &[&str], key: &str, up: bool) -> Vec<String> {
    let Some(pos) = present.iter().position(|k| *k == key) else {
        return order.to_vec();
    };
    let target = if up {
        let Some(prev) = pos.checked_sub(1) else {
            return order.to_vec();
        };
        prev
    } else {
        if pos + 1 >= present.len() {
            return order.to_vec();
        }
        pos + 1
    };
    let other = present[target];
    if other == key {
        return order.to_vec();
    }
    let mut out = order.to_vec();
    for &k in present {
        if !out.iter().any(|existing| existing == k) {
            out.push(k.to_owned());
        }
    }
    if let (Some(a), Some(b)) = (
        out.iter().position(|k| k == key),
        out.iter().position(|k| k == other),
    ) {
        out.swap(a, b);
    }
    out
}

/// Moves `from_key` to the slot of `to_key` among `present`, preserving any
/// previously ordered keys that aren't currently running.
pub fn move_key_to(order: &[String], present: &[&str], from_key: &str, to_key: &str) -> Vec<String> {
    let (Some(from_pos), Some(to_pos)) = (
        present.iter().position(|k| *k == from_key),
        present.iter().position(|k| *k == to_key),
    ) else {
        return order.to_vec();
    };
    if from_pos == to_pos {
        return order.to_vec();
    }
    let mut out: Vec<String> = present.iter().map(|k| (*k).to_owned()).collect();
    let moved = out.remove(from_pos);
    out.insert(to_pos, moved);
    for existing in order {
        if !out.contains(existing) {
            out.push(existing.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn new_apps_are_visible_by_default_and_toggle_hides_then_shows() {
        let hidden: Vec<String> = Vec::new();
        assert!(is_key_visible(&hidden, "discord"));

        let hidden = toggle_visible(&hidden, "discord");
        assert!(!is_key_visible(&hidden, "discord"));
        assert!(is_key_visible(&hidden, "steam"));
        assert_eq!(hidden, s(&["discord"]));

        let hidden = toggle_visible(&hidden, "discord");
        assert!(is_key_visible(&hidden, "discord"));
        assert!(hidden.is_empty());
    }

    #[test]
    fn toggle_visible_removes_duplicate_entries() {
        let hidden = s(&["a", "b", "a"]);
        assert_eq!(toggle_visible(&hidden, "a"), s(&["b"]));
    }

    #[test]
    fn ordered_indices_follows_saved_order_then_new_items() {
        let items = [
            (1, Some("steam")),
            (2, Some("discord")),
            (3, None),
            (4, Some("obs")),
        ];
        assert_eq!(ordered_indices(&items, &s(&["obs", "steam"])), vec![4, 1, 2, 3]);
    }

    #[test]
    fn move_visible_reorders_from_empty_and_saved_order() {
        let order = move_visible(&[], &["a", "b", "c"], "c", true);
        assert_eq!(order, s(&["a", "c", "b"]));

        let order = move_visible(&order, &["a", "c", "b"], "a", false);
        assert_eq!(order, s(&["c", "a", "b"]));

        // Boundaries are no-ops.
        assert_eq!(move_visible(&order, &["c", "a", "b"], "c", true), order);
        assert_eq!(move_visible(&order, &["c", "a", "b"], "b", false), order);
        assert_eq!(move_visible(&order, &["c", "a", "b"], "missing", true), order);
    }

    #[test]
    fn move_visible_preserves_absent_apps_in_order() {
        let order = s(&["a", "offline", "b"]);
        let moved = move_visible(&order, &["a", "b"], "b", true);
        assert_eq!(moved, s(&["b", "offline", "a"]));
        let items = [(1, Some("a")), (2, Some("b"))];
        assert_eq!(ordered_indices(&items, &moved), vec![2, 1]);
    }
}
