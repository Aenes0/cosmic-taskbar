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
    /// What the app's icon does: its own action, or its menu where it has
    /// nothing else.
    Activate(usize),
    /// The app's menu, as a page of the panel.
    OpenMenu(usize),
    /// Puts the app on the bar, or takes it off.
    Toggle(usize),
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

    /// Every app, by its id here.
    pub fn apps(&self) -> impl Iterator<Item = (usize, &status_menu::State)> {
        self.items.iter().map(|(id, state)| (*id, state))
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

    /// Whether the app is only a menu: no action of its own.
    pub fn is_menu(&self, id: usize) -> Option<bool> {
        Some(self.items.get(&id)?.item.is_menu())
    }

    pub fn is_visible(&self, id: usize) -> bool {
        self.key_of(id).is_some_and(|key| self.config.visible.iter().any(|k| k == key))
    }

    /// The apps picked for the bar that are there now, in the order picked:
    /// id, icon and name.
    pub fn on_bar(&self) -> Vec<(usize, icon::Handle, String)> {
        self.config
            .visible
            .iter()
            .filter_map(|key| {
                let (id, state) = self
                    .items
                    .iter()
                    .find(|(_, state)| state.identity.key() == Some(key.as_str()))?;
                let name = state.identity.label().unwrap_or(key.as_str()).to_owned();
                Some((*id, state.icon_handle().clone(), name))
            })
            .collect()
    }

    /// An app on the bar, or off it.
    pub fn toggle(&mut self, id: usize) {
        let Some(key) = self.key_of(id).map(str::to_owned) else {
            return;
        };
        let mut visible = self.config.visible.clone();
        match visible.iter().position(|k| *k == key) {
            Some(at) => {
                visible.remove(at);
            }
            None => visible.push(key),
        }
        self.set_visible(visible);
    }

    /// Writes the list: the other processes, the bar's too, get it from the
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

    /// The app's own action, with the token the compositor gives for it.
    pub fn activate(&self, id: usize) -> app::Task<Message> {
        let Some(state) = self.items.get(&id) else {
            return Task::none();
        };
        if let Some(token_tx) = &self.token_tx {
            let _ = token_tx.send(TokenRequest {
                app_id: crate::central::tray::APP_ID.to_string(),
                exec: format!("activate:{id}"),
            });
            return Task::none();
        }
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
                if let Some(id) = exec.strip_prefix("activate:") {
                    if let Ok(id) = id.parse::<usize>()
                        && let Some(state) = self.items.get(&id)
                    {
                        return activate(id, &state.item, token);
                    }
                    return Task::none();
                }
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
