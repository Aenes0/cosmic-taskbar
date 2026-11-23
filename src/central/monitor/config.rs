// SPDX-License-Identifier: GPL-3.0-only

//! What the user picks for the monitor: what the bar shows, the panels'
//! order, which are hidden or folded, and the energy's price.

use std::collections::HashMap;

use cosmic::cosmic_config::{Config, ConfigGet, ConfigSet};

use super::texts::texts;

fn config() -> Option<Config> {
    Config::new(super::APP_ID, 1).ok()
}

/// When the settings were last saved, by whichever process: the panels' host
/// saves what is chosen in the panel, the bar's applets show it. A few file
/// dates, at each reading: no watcher thread for something that changes by hand.
pub fn modified() -> Option<std::time::SystemTime> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config")))?;
    newest_in(&base.join("cosmic").join(super::APP_ID).join("v1"))
}

/// The newest date of the files in `dir`.
fn newest_in(dir: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok()?.metadata().ok()?.modified().ok())
        .max()
}

fn get<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    config()?.get::<T>(key).ok()
}

fn set<T: serde::Serialize>(key: &str, value: T) {
    if let Some(Err(err)) = config().map(|c| c.set(key, value)) {
        tracing::warn!("cannot save {key}: {err}");
    }
}

/// A panel of the popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panel {
    Energy,
    Cpu,
    Gpu,
    Memory,
    Disks,
    Network,
    Sensors,
    Processes,
    System,
}

impl Panel {
    pub const ALL: [Panel; 9] = [
        Panel::Energy,
        Panel::Cpu,
        Panel::Gpu,
        Panel::Memory,
        Panel::Disks,
        Panel::Network,
        Panel::Sensors,
        Panel::Processes,
        Panel::System,
    ];

    fn id(self) -> &'static str {
        match self {
            Panel::Energy => "energy",
            Panel::Cpu => "cpu",
            Panel::Gpu => "gpu",
            Panel::Memory => "memory",
            Panel::Disks => "disks",
            Panel::Network => "network",
            Panel::Sensors => "sensors",
            Panel::Processes => "processes",
            Panel::System => "system",
        }
    }

    fn from_id(id: &str) -> Option<Panel> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            Panel::Energy => t.energy,
            Panel::Cpu => t.cpu,
            Panel::Gpu => t.gpu,
            Panel::Memory => t.memory,
            Panel::Disks => t.disks,
            Panel::Network => t.network,
            Panel::Sensors => t.sensors,
            Panel::Processes => t.processes,
            Panel::System => t.system,
        }
    }
}

/// Something the bar can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarItem {
    Weather,
    Cpu,
    Gpu,
    Memory,
    Power,
    Network,
}

impl BarItem {
    pub const ALL: [BarItem; 6] =
        [BarItem::Weather, BarItem::Cpu, BarItem::Gpu, BarItem::Memory, BarItem::Power, BarItem::Network];

    fn id(self) -> &'static str {
        match self {
            BarItem::Weather => "weather",
            BarItem::Cpu => "cpu",
            BarItem::Gpu => "gpu",
            BarItem::Memory => "memory",
            BarItem::Power => "power",
            BarItem::Network => "network",
        }
    }

    /// Shown never, always, or past its limit; the rest, on or off. The
    /// energy always shows.
    pub fn has_modes(self) -> bool {
        matches!(self, BarItem::Cpu | BarItem::Gpu | BarItem::Memory)
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            BarItem::Weather => t.bar_weather,
            BarItem::Cpu => t.bar_cpu,
            BarItem::Gpu => t.bar_gpu,
            BarItem::Memory => t.memory,
            BarItem::Power => t.bar_power,
            BarItem::Network => t.bar_network,
        }
    }
}

/// When an item with modes shows on the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMode {
    Never,
    Always,
    /// Past [`USAGE_LIMIT`] or [`TEMP_LIMIT`]; memory, past [`USAGE_LIMIT`]
    /// full.
    Over,
}

/// Percent of use, or of memory full, past which an item shows.
pub const USAGE_LIMIT: f32 = 70.0;
/// Degrees, for the CPU and the GPU.
pub const TEMP_LIMIT: f32 = 65.0;

impl BarMode {
    pub const ALL: [BarMode; 3] = [BarMode::Never, BarMode::Always, BarMode::Over];

    fn id(self) -> &'static str {
        match self {
            BarMode::Never => "never",
            BarMode::Always => "always",
            BarMode::Over => "over",
        }
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            BarMode::Never => t.bar_never,
            BarMode::Always => t.bar_always,
            BarMode::Over => t.bar_over,
        }
    }
}

/// Display mode for a card in the monitor panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardMode {
    /// Expanded view with full charts and details.
    Full,
    /// Intermediate compact view: key numbers + progress/summary bar without large charts.
    Compact,
    /// Reduced/collapsed view: single header row with inline summary beside the title.
    Minimal,
    /// Hidden from the monitor panel.
    Hidden,
}

impl CardMode {
    pub const ALL: [CardMode; 4] = [
        CardMode::Full,
        CardMode::Compact,
        CardMode::Minimal,
        CardMode::Hidden,
    ];

    pub fn id(self) -> &'static str {
        match self {
            CardMode::Full => "full",
            CardMode::Compact => "compact",
            CardMode::Minimal => "minimal",
            CardMode::Hidden => "hidden",
        }
    }

    pub fn from_id(id: &str) -> Option<CardMode> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            CardMode::Full => t.mode_full,
            CardMode::Compact => t.mode_compact,
            CardMode::Minimal => t.mode_minimal,
            CardMode::Hidden => t.mode_hidden,
        }
    }

    /// Cycles through the three visible modes: Full -> Compact -> Minimal -> Full.
    pub fn next_visible(self) -> CardMode {
        match self {
            CardMode::Full => CardMode::Compact,
            CardMode::Compact => CardMode::Minimal,
            CardMode::Minimal | CardMode::Hidden => CardMode::Full,
        }
    }
}

/// Active sort column for the Processes card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ProcSort {
    #[default]
    Cpu,
    Ram,
    Gpu,
    Name,
}

impl ProcSort {
    pub const ALL: [ProcSort; 4] = [ProcSort::Cpu, ProcSort::Ram, ProcSort::Gpu, ProcSort::Name];

    pub fn id(self) -> &'static str {
        match self {
            ProcSort::Cpu => "cpu",
            ProcSort::Ram => "ram",
            ProcSort::Gpu => "gpu",
            ProcSort::Name => "name",
        }
    }

    pub fn from_id(id: &str) -> Option<ProcSort> {
        Self::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            ProcSort::Cpu => t.cpu,
            ProcSort::Ram => t.ram_col,
            ProcSort::Gpu => t.gpu,
            ProcSort::Name => t.name_col,
        }
    }

    /// Default sort direction for each column: A->Z (`true`) for Name,
    /// descending (`false`) for CPU, RAM and GPU.
    pub fn default_asc(self) -> bool {
        matches!(self, ProcSort::Name)
    }
}

/// Telemetry component with an independent sampling interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PollComponent {
    Cpu,
    Gpu,
    Ram,
    Disk,
    Net,
    Procs,
    Weather,
}

impl PollComponent {
    pub const ALL: [PollComponent; 7] = [
        PollComponent::Cpu,
        PollComponent::Gpu,
        PollComponent::Ram,
        PollComponent::Disk,
        PollComponent::Net,
        PollComponent::Procs,
        PollComponent::Weather,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PollComponent::Cpu => "cpu",
            PollComponent::Gpu => "gpu",
            PollComponent::Ram => "ram",
            PollComponent::Disk => "disk",
            PollComponent::Net => "net",
            PollComponent::Procs => "procs",
            PollComponent::Weather => "weather",
        }
    }

    pub fn name(self) -> &'static str {
        let t = texts();
        match self {
            PollComponent::Cpu => t.cpu,
            PollComponent::Gpu => t.gpu,
            PollComponent::Ram => t.memory,
            PollComponent::Disk => t.disks,
            PollComponent::Net => t.network,
            PollComponent::Procs => t.processes,
            PollComponent::Weather => t.weather,
        }
    }
}

/// Suggested per-component polling intervals in seconds.
pub const POLL_SECS_OPTIONS: [u64; 5] = [1, 2, 5, 10, 30];
/// Suggested weather refresh intervals in minutes.
pub const WEATHER_MINS_OPTIONS: [u64; 3] = [15, 30, 60];
/// Suggested visible process counts.
pub const PROCS_COUNT_OPTIONS: [usize; 5] = [3, 5, 8, 10, 15];

#[derive(Clone, Debug)]
pub struct Settings {
    /// The panels, in order.
    pub order: Vec<Panel>,
    pub hidden: Vec<Panel>,
    pub folded: Vec<Panel>,
    /// Display mode for each card (Full, Compact, Minimal, Hidden).
    pub card_modes: HashMap<Panel, CardMode>,
    /// Display mode for the top weather card.
    pub weather_mode: CardMode,
    /// What the bar leaves out, of those on or off.
    pub bar_hidden: Vec<BarItem>,
    /// Of those with modes, the ones set.
    pub bar_modes: HashMap<BarItem, BarMode>,
    /// The red thermometer beside the CPU's and the GPU's icons.
    pub thermometer: BarMode,
    /// Price of a kWh, in the local money.
    pub tariff: Option<f64>,
    /// Watts the rest of the computer takes, guessed: board, memory,
    /// drives, fans, and what the power supply loses.
    pub rest_watts: f64,
    /// The weather's details and days shown, not just now and the hours.
    pub weather_expanded: bool,
    /// Background polling interval in seconds for the bar applet (1..=30).
    pub poll_secs: u64,
    /// Per-component polling intervals in seconds (1..=60).
    pub cpu_secs: u64,
    pub gpu_secs: u64,
    pub ram_secs: u64,
    pub disk_secs: u64,
    pub net_secs: u64,
    pub procs_secs: u64,
    /// Weather refresh interval in minutes (5..=180).
    pub weather_mins: u64,
    /// Number of processes shown in the Processes card (1..=30).
    pub procs_count: usize,
    /// Active sort column for the Processes card.
    pub proc_sort: ProcSort,
    /// Whether the Processes card sorts ascending (`true`) or descending (`false`).
    pub proc_sort_asc: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            order: Panel::ALL.to_vec(),
            hidden: Vec::new(),
            folded: Vec::new(),
            card_modes: HashMap::new(),
            weather_mode: CardMode::Compact,
            bar_hidden: Vec::new(),
            bar_modes: HashMap::new(),
            thermometer: BarMode::Over,
            tariff: None,
            rest_watts: 0.0,
            weather_expanded: false,
            poll_secs: 5,
            cpu_secs: 1,
            gpu_secs: 2,
            ram_secs: 2,
            disk_secs: 2,
            net_secs: 1,
            procs_secs: 2,
            weather_mins: 30,
            procs_count: 5,
            proc_sort: ProcSort::Cpu,
            proc_sort_asc: false,
        }
    }
}

fn panels(key: &str) -> Option<Vec<Panel>> {
    get::<Vec<String>>(key).map(|ids| ids.iter().filter_map(|id| Panel::from_id(id)).collect())
}

fn save_panels(key: &str, panels: &[Panel]) {
    set(key, panels.iter().map(|p| p.id()).collect::<Vec<_>>());
}

/// Migrates legacy `hidden` + `folded` lists into [`CardMode`] while letting
/// explicitly saved `card_modes` take precedence.
fn resolve_card_modes(
    saved: Option<&HashMap<String, String>>,
    hidden: &[Panel],
    folded: &[Panel],
) -> HashMap<Panel, CardMode> {
    let mut modes = HashMap::with_capacity(Panel::ALL.len());
    for panel in Panel::ALL {
        let mode = saved
            .and_then(|map| map.get(panel.id()))
            .and_then(|id| CardMode::from_id(id))
            .unwrap_or_else(|| {
                if hidden.contains(&panel) {
                    CardMode::Hidden
                } else if folded.contains(&panel) {
                    CardMode::Minimal
                } else {
                    CardMode::Full
                }
            });
        modes.insert(panel, mode);
    }
    modes
}

/// Pure reordering helper used by [`Settings::move_card_to`] and unit tests.
fn move_in_order(order: &mut Vec<Panel>, from: Panel, to: Panel) -> bool {
    if from == to {
        return false;
    }
    let (Some(from_idx), Some(to_idx)) = (
        order.iter().position(|p| *p == from),
        order.iter().position(|p| *p == to),
    ) else {
        return false;
    };
    let item = order.remove(from_idx);
    order.insert(to_idx, item);
    true
}

impl Settings {
    pub fn load() -> Settings {
        let defaults = Settings::default();
        let mut order = panels("order").unwrap_or_default();
        // New ones at the end.
        for panel in Panel::ALL {
            if !order.contains(&panel) {
                order.push(panel);
            }
        }
        let legacy_hidden = panels("hidden").unwrap_or_default();
        let legacy_folded = panels("folded").unwrap_or_default();
        let saved_modes = get::<HashMap<String, String>>("card_modes");
        let card_modes = resolve_card_modes(saved_modes.as_ref(), &legacy_hidden, &legacy_folded);
        let hidden: Vec<Panel> = Panel::ALL
            .into_iter()
            .filter(|p| card_modes.get(p) == Some(&CardMode::Hidden))
            .collect();
        let folded: Vec<Panel> = Panel::ALL
            .into_iter()
            .filter(|p| card_modes.get(p) == Some(&CardMode::Minimal))
            .collect();

        let bar_hidden = get::<Vec<String>>("bar_hidden")
            .unwrap_or_default()
            .iter()
            .filter_map(|id| BarItem::ALL.into_iter().find(|b| b.id() == id))
            .collect::<Vec<_>>();
        let mut bar_modes: HashMap<BarItem, BarMode> = get::<HashMap<String, String>>("bar_modes")
            .unwrap_or_default()
            .iter()
            .filter_map(|(item, mode)| {
                let item = BarItem::ALL.into_iter().find(|b| b.id() == item)?;
                let mode = BarMode::ALL.into_iter().find(|m| m.id() == mode)?;
                Some((item, mode))
            })
            .collect();
        // Hidden before there were modes: still.
        for item in &bar_hidden {
            if item.has_modes() {
                bar_modes.entry(*item).or_insert(BarMode::Never);
            }
        }

        let legacy_weather_expanded = get::<bool>("weather_expanded").unwrap_or(false);
        let weather_mode = get::<String>("weather_mode")
            .and_then(|id| CardMode::from_id(&id))
            .unwrap_or(if legacy_weather_expanded {
                CardMode::Full
            } else {
                CardMode::Compact
            });
        let weather_expanded = weather_mode == CardMode::Full;

        let poll_secs = get::<u64>("poll_secs").unwrap_or(defaults.poll_secs).clamp(1, 30);
        let proc_sort = get::<String>("proc_sort")
            .and_then(|id| ProcSort::from_id(&id))
            .unwrap_or(defaults.proc_sort);
        let proc_sort_asc = get::<bool>("proc_sort_asc").unwrap_or_else(|| proc_sort.default_asc());

        Settings {
            order,
            hidden,
            folded,
            card_modes,
            weather_mode,
            bar_hidden: bar_hidden.into_iter().filter(|b| !b.has_modes()).collect(),
            bar_modes,
            thermometer: get::<String>("bar_thermometer")
                .and_then(|id| BarMode::ALL.into_iter().find(|m| m.id() == id))
                .unwrap_or(BarMode::Over),
            tariff: get::<f64>("tariff").filter(|t| *t > 0.0),
            rest_watts: get::<f64>("rest_watts").unwrap_or(0.0).max(0.0),
            weather_expanded,
            poll_secs,
            cpu_secs: get::<u64>("cpu_secs").unwrap_or(defaults.cpu_secs).clamp(1, 60),
            gpu_secs: get::<u64>("gpu_secs").unwrap_or(defaults.gpu_secs).clamp(1, 60),
            ram_secs: get::<u64>("ram_secs").unwrap_or(defaults.ram_secs).clamp(1, 60),
            disk_secs: get::<u64>("disk_secs").unwrap_or(defaults.disk_secs).clamp(1, 60),
            net_secs: get::<u64>("net_secs").unwrap_or(defaults.net_secs).clamp(1, 60),
            procs_secs: get::<u64>("procs_secs").unwrap_or(defaults.procs_secs).clamp(1, 60),
            weather_mins: get::<u64>("weather_mins").unwrap_or(defaults.weather_mins).clamp(5, 180),
            procs_count: get::<u64>("procs_count")
                .map(|n| n as usize)
                .unwrap_or(defaults.procs_count)
                .clamp(1, 30),
            proc_sort,
            proc_sort_asc,
        }
    }

    pub fn card_mode(&self, panel: Panel) -> CardMode {
        if let Some(&mode) = self.card_modes.get(&panel) {
            return mode;
        }
        if self.hidden.contains(&panel) {
            CardMode::Hidden
        } else if self.folded.contains(&panel) {
            CardMode::Minimal
        } else {
            CardMode::Full
        }
    }

    fn apply_card_mode(&mut self, panel: Panel, mode: CardMode) {
        self.card_modes.insert(panel, mode);
        self.hidden.retain(|p| *p != panel);
        self.folded.retain(|p| *p != panel);
        match mode {
            CardMode::Hidden => self.hidden.push(panel),
            CardMode::Minimal => self.folded.push(panel),
            CardMode::Full | CardMode::Compact => {}
        }
    }

    fn save_card_modes(&self) {
        let map: HashMap<&str, &str> = Panel::ALL
            .into_iter()
            .map(|p| (p.id(), self.card_mode(p).id()))
            .collect();
        set("card_modes", map);
        save_panels("hidden", &self.hidden);
        save_panels("folded", &self.folded);
    }

    pub fn set_card_mode(&mut self, panel: Panel, mode: CardMode) {
        self.apply_card_mode(panel, mode);
        self.save_card_modes();
    }

    /// Cycles a card through the three visible modes (Full -> Compact -> Minimal -> Full).
    pub fn cycle_card_mode(&mut self, panel: Panel) -> CardMode {
        let next = self.card_mode(panel).next_visible();
        self.set_card_mode(panel, next);
        next
    }

    pub fn is_hidden(&self, panel: Panel) -> bool {
        self.card_mode(panel) == CardMode::Hidden
    }

    pub fn is_folded(&self, panel: Panel) -> bool {
        self.card_mode(panel) == CardMode::Minimal
    }

    pub fn toggle_fold(&mut self, panel: Panel) {
        let next = if self.card_mode(panel) == CardMode::Minimal {
            CardMode::Full
        } else {
            CardMode::Minimal
        };
        self.set_card_mode(panel, next);
    }

    pub fn toggle_hidden(&mut self, panel: Panel) {
        let next = if self.card_mode(panel) == CardMode::Hidden {
            CardMode::Full
        } else {
            CardMode::Hidden
        };
        self.set_card_mode(panel, next);
    }

    /// A panel a place up or down.
    pub fn shift(&mut self, panel: Panel, down: bool) {
        let Some(at) = self.order.iter().position(|p| *p == panel) else { return };
        let to = if down { at + 1 } else { at.wrapping_sub(1) };
        if to < self.order.len() {
            self.order.swap(at, to);
            save_panels("order", &self.order);
        }
    }

    /// Moves `from` to the position of `to` in `self.order` (drag-and-drop).
    pub fn move_card_to(&mut self, from: Panel, to: Panel) {
        if move_in_order(&mut self.order, from, to) {
            save_panels("order", &self.order);
        }
    }

    pub fn weather_mode(&self) -> CardMode {
        self.weather_mode
    }

    pub fn set_weather_mode(&mut self, mode: CardMode) {
        self.weather_mode = mode;
        self.weather_expanded = mode == CardMode::Full;
        set("weather_mode", mode.id());
        set("weather_expanded", self.weather_expanded);
    }

    pub fn cycle_weather_mode(&mut self) -> CardMode {
        let next = match self.weather_mode {
            CardMode::Compact => CardMode::Full,
            CardMode::Full => CardMode::Minimal,
            CardMode::Minimal | CardMode::Hidden => CardMode::Compact,
        };
        self.set_weather_mode(next);
        next
    }

    /// Of those on or off.
    pub fn shows_on_bar(&self, item: BarItem) -> bool {
        item == BarItem::Power || !self.bar_hidden.contains(&item)
    }

    pub fn toggle_bar(&mut self, item: BarItem) {
        toggle(&mut self.bar_hidden, item);
        set("bar_hidden", self.bar_hidden.iter().map(|b| b.id()).collect::<Vec<_>>());
    }

    /// Of those with modes: past the limit, unless set.
    pub fn bar_mode(&self, item: BarItem) -> BarMode {
        self.bar_modes.get(&item).copied().unwrap_or(BarMode::Over)
    }

    pub fn set_thermometer(&mut self, mode: BarMode) {
        self.thermometer = mode;
        set("bar_thermometer", mode.id());
    }

    pub fn set_bar_mode(&mut self, item: BarItem, mode: BarMode) {
        self.bar_modes.insert(item, mode);
        self.save_bar_modes();
    }

    fn save_bar_modes(&self) {
        let modes: HashMap<&str, &str> =
            self.bar_modes.iter().map(|(item, mode)| (item.id(), mode.id())).collect();
        set("bar_modes", modes);
        // What came before modes, emptied of theirs.
        set("bar_hidden", self.bar_hidden.iter().map(|b| b.id()).collect::<Vec<_>>());
    }

    pub fn toggle_weather_expanded(&mut self) {
        let next = if self.weather_mode == CardMode::Full {
            CardMode::Compact
        } else {
            CardMode::Full
        };
        self.set_weather_mode(next);
    }

    pub fn set_tariff(&mut self, tariff: Option<f64>) {
        self.tariff = tariff.filter(|t| *t > 0.0);
        set("tariff", self.tariff.unwrap_or(0.0));
    }

    pub fn set_rest_watts(&mut self, watts: f64) {
        self.rest_watts = watts.max(0.0);
        set("rest_watts", self.rest_watts);
    }

    pub fn set_poll_secs(&mut self, secs: u64) {
        self.poll_secs = secs.clamp(1, 30);
        set("poll_secs", self.poll_secs);
    }

    /// Returns the configured polling interval for a component (seconds for
    /// hardware/processes, minutes for weather).
    pub fn component_poll(&self, component: PollComponent) -> u64 {
        match component {
            PollComponent::Cpu => self.cpu_secs,
            PollComponent::Gpu => self.gpu_secs,
            PollComponent::Ram => self.ram_secs,
            PollComponent::Disk => self.disk_secs,
            PollComponent::Net => self.net_secs,
            PollComponent::Procs => self.procs_secs,
            PollComponent::Weather => self.weather_mins,
        }
    }

    fn apply_component_poll(&mut self, component: PollComponent, value: u64) -> (&'static str, u64) {
        match component {
            PollComponent::Cpu => {
                self.cpu_secs = value.clamp(1, 60);
                ("cpu_secs", self.cpu_secs)
            }
            PollComponent::Gpu => {
                self.gpu_secs = value.clamp(1, 60);
                ("gpu_secs", self.gpu_secs)
            }
            PollComponent::Ram => {
                self.ram_secs = value.clamp(1, 60);
                ("ram_secs", self.ram_secs)
            }
            PollComponent::Disk => {
                self.disk_secs = value.clamp(1, 60);
                ("disk_secs", self.disk_secs)
            }
            PollComponent::Net => {
                self.net_secs = value.clamp(1, 60);
                ("net_secs", self.net_secs)
            }
            PollComponent::Procs => {
                self.procs_secs = value.clamp(1, 60);
                ("procs_secs", self.procs_secs)
            }
            PollComponent::Weather => {
                self.weather_mins = value.clamp(5, 180);
                ("weather_mins", self.weather_mins)
            }
        }
    }

    /// Sets and persists the polling interval for a component (seconds for
    /// CPU/GPU/RAM/Disk/Net/Procs, minutes for Weather).
    pub fn set_component_poll(&mut self, component: PollComponent, value: u64) {
        let (key, clamped) = self.apply_component_poll(component, value);
        set(key, clamped);
    }

    pub fn set_cpu_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Cpu, secs);
    }

    pub fn set_gpu_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Gpu, secs);
    }

    pub fn set_ram_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Ram, secs);
    }

    pub fn set_disk_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Disk, secs);
    }

    pub fn set_net_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Net, secs);
    }

    pub fn set_procs_secs(&mut self, secs: u64) {
        self.set_component_poll(PollComponent::Procs, secs);
    }

    pub fn set_weather_mins(&mut self, mins: u64) {
        self.set_component_poll(PollComponent::Weather, mins);
    }

    fn apply_procs_count(&mut self, count: usize) {
        self.procs_count = count.clamp(1, 30);
    }

    pub fn set_procs_count(&mut self, count: usize) {
        self.apply_procs_count(count);
        set("procs_count", self.procs_count as u64);
    }

    fn apply_proc_sort(&mut self, sort: ProcSort) {
        self.proc_sort = sort;
        self.proc_sort_asc = sort.default_asc();
    }

    pub fn set_proc_sort(&mut self, sort: ProcSort) {
        self.apply_proc_sort(sort);
        set("proc_sort", self.proc_sort.id());
        set("proc_sort_asc", self.proc_sort_asc);
    }

    fn apply_click_proc_sort(&mut self, sort: ProcSort) {
        if self.proc_sort == sort {
            self.proc_sort_asc = !self.proc_sort_asc;
        } else {
            self.proc_sort = sort;
            self.proc_sort_asc = sort.default_asc();
        }
    }

    /// Handles clicking a column header in the Processes table: switches to
    /// that column with its default direction, or flips ascending/descending
    /// if already active.
    pub fn click_proc_sort(&mut self, sort: ProcSort) {
        self.apply_click_proc_sort(sort);
        set("proc_sort", self.proc_sort.id());
        set("proc_sort_asc", self.proc_sort_asc);
    }

    /// The order, the panels and the bar as they come.
    pub fn reset(&mut self) {
        let defaults = Settings::default();
        self.order = defaults.order;
        self.hidden.clear();
        self.folded.clear();
        self.card_modes.clear();
        self.weather_mode = defaults.weather_mode;
        self.weather_expanded = defaults.weather_expanded;
        self.bar_hidden.clear();
        self.bar_modes.clear();
        self.thermometer = defaults.thermometer;
        self.poll_secs = defaults.poll_secs;
        self.cpu_secs = defaults.cpu_secs;
        self.gpu_secs = defaults.gpu_secs;
        self.ram_secs = defaults.ram_secs;
        self.disk_secs = defaults.disk_secs;
        self.net_secs = defaults.net_secs;
        self.procs_secs = defaults.procs_secs;
        self.weather_mins = defaults.weather_mins;
        self.procs_count = defaults.procs_count;
        self.proc_sort = defaults.proc_sort;
        self.proc_sort_asc = defaults.proc_sort_asc;
        set("bar_thermometer", self.thermometer.id());
        set("poll_secs", self.poll_secs);
        set("cpu_secs", self.cpu_secs);
        set("gpu_secs", self.gpu_secs);
        set("ram_secs", self.ram_secs);
        set("disk_secs", self.disk_secs);
        set("net_secs", self.net_secs);
        set("procs_secs", self.procs_secs);
        set("weather_mins", self.weather_mins);
        set("procs_count", self.procs_count as u64);
        set("proc_sort", self.proc_sort.id());
        set("proc_sort_asc", self.proc_sort_asc);
        set("weather_mode", self.weather_mode.id());
        set("weather_expanded", self.weather_expanded);
        save_panels("order", &self.order);
        self.save_card_modes();
        self.save_bar_modes();
    }
}

fn toggle<T: PartialEq>(list: &mut Vec<T>, item: T) {
    match list.iter().position(|i| *i == item) {
        Some(at) => {
            list.remove(at);
        }
        None => list.push(item),
    }
}

/// Where the weather is, as last chosen on any screen's applet; at first,
/// CEP 30330-330.
pub fn weather_place() -> super::weather::Place {
    get::<super::weather::Place>("weather_place").unwrap_or_default()
}

pub fn set_weather_place(place: &super::weather::Place) {
    set("weather_place", place);
}

#[cfg(test)]
mod modified_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn the_newest_file_s_date_is_the_settings_date() {
        let dir = std::env::temp_dir().join(format!("monitor-settings-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&dir);
        assert!(newest_in(&dir).is_none(), "no settings yet");
        std::fs::create_dir_all(&dir).unwrap();
        let old = std::fs::File::create(dir.join("order")).unwrap();
        old.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000)).unwrap();
        let new = std::fs::File::create(dir.join("bar_modes")).unwrap();
        new.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000)).unwrap();
        assert_eq!(newest_in(&dir), Some(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000)));
        // A key saved again: a newer date.
        new.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(3_000)).unwrap();
        assert_eq!(newest_in(&dir), Some(SystemTime::UNIX_EPOCH + Duration::from_secs(3_000)));
        _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn card_mode_migration_and_sync() {
        let legacy_hidden = vec![Panel::System];
        let legacy_folded = vec![Panel::Disks, Panel::Sensors];
        let migrated = resolve_card_modes(None, &legacy_hidden, &legacy_folded);
        assert_eq!(migrated[&Panel::System], CardMode::Hidden);
        assert_eq!(migrated[&Panel::Disks], CardMode::Minimal);
        assert_eq!(migrated[&Panel::Sensors], CardMode::Minimal);
        assert_eq!(migrated[&Panel::Cpu], CardMode::Full);

        // Explicitly saved card_modes take precedence over legacy lists.
        let mut saved = HashMap::new();
        saved.insert("cpu".to_owned(), "compact".to_owned());
        saved.insert("disks".to_owned(), "full".to_owned());
        let resolved = resolve_card_modes(Some(&saved), &legacy_hidden, &legacy_folded);
        assert_eq!(resolved[&Panel::Cpu], CardMode::Compact);
        assert_eq!(resolved[&Panel::Disks], CardMode::Full);
        assert_eq!(resolved[&Panel::System], CardMode::Hidden);
        assert_eq!(resolved[&Panel::Sensors], CardMode::Minimal);

        // next_visible cycles Full -> Compact -> Minimal -> Full.
        assert_eq!(CardMode::Full.next_visible(), CardMode::Compact);
        assert_eq!(CardMode::Compact.next_visible(), CardMode::Minimal);
        assert_eq!(CardMode::Minimal.next_visible(), CardMode::Full);
        assert_eq!(CardMode::Hidden.next_visible(), CardMode::Full);

        // apply_card_mode keeps hidden & folded synchronized.
        let mut s = Settings::default();
        s.apply_card_mode(Panel::Gpu, CardMode::Hidden);
        assert!(s.is_hidden(Panel::Gpu));
        assert!(s.hidden.contains(&Panel::Gpu));
        assert!(!s.folded.contains(&Panel::Gpu));

        s.apply_card_mode(Panel::Gpu, CardMode::Minimal);
        assert!(s.is_folded(Panel::Gpu));
        assert!(!s.hidden.contains(&Panel::Gpu));
        assert!(s.folded.contains(&Panel::Gpu));

        s.apply_card_mode(Panel::Gpu, CardMode::Compact);
        assert_eq!(s.card_mode(Panel::Gpu), CardMode::Compact);
        assert!(!s.hidden.contains(&Panel::Gpu));
        assert!(!s.folded.contains(&Panel::Gpu));
    }

    #[test]
    fn move_card_to_reorders_forward_and_backward() {
        let mut order = vec![Panel::Energy, Panel::Cpu, Panel::Gpu, Panel::Memory];
        assert!(move_in_order(&mut order, Panel::Energy, Panel::Gpu));
        assert_eq!(order, vec![Panel::Cpu, Panel::Gpu, Panel::Energy, Panel::Memory]);

        assert!(move_in_order(&mut order, Panel::Memory, Panel::Cpu));
        assert_eq!(order, vec![Panel::Memory, Panel::Cpu, Panel::Gpu, Panel::Energy]);

        // Moving onto itself is a no-op.
        assert!(!move_in_order(&mut order, Panel::Cpu, Panel::Cpu));
        assert_eq!(order, vec![Panel::Memory, Panel::Cpu, Panel::Gpu, Panel::Energy]);

        // Moving an absent item is a no-op.
        assert!(!move_in_order(&mut order, Panel::System, Panel::Cpu));
    }

    #[test]
    fn per_component_poll_intervals_clamp_and_roundtrip() {
        let mut s = Settings::default();
        assert_eq!(s.component_poll(PollComponent::Cpu), 1);
        assert_eq!(s.component_poll(PollComponent::Weather), 30);

        s.apply_component_poll(PollComponent::Cpu, 0);
        assert_eq!(s.cpu_secs, 1);
        s.apply_component_poll(PollComponent::Gpu, 10);
        assert_eq!(s.gpu_secs, 10);
        s.apply_component_poll(PollComponent::Ram, 999);
        assert_eq!(s.ram_secs, 60);
        s.apply_component_poll(PollComponent::Disk, 5);
        assert_eq!(s.disk_secs, 5);
        s.apply_component_poll(PollComponent::Net, 2);
        assert_eq!(s.net_secs, 2);
        s.apply_component_poll(PollComponent::Procs, 30);
        assert_eq!(s.procs_secs, 30);
        s.apply_component_poll(PollComponent::Weather, 1);
        assert_eq!(s.weather_mins, 5);
        s.apply_component_poll(PollComponent::Weather, 60);
        assert_eq!(s.weather_mins, 60);
        s.apply_component_poll(PollComponent::Weather, 500);
        assert_eq!(s.weather_mins, 180);
    }

    #[test]
    fn proc_sort_and_count_defaults_and_toggling() {
        let mut s = Settings::default();
        assert_eq!(s.procs_count, 5);
        assert_eq!(s.proc_sort, ProcSort::Cpu);
        assert!(!s.proc_sort_asc);

        s.apply_procs_count(0);
        assert_eq!(s.procs_count, 1);
        s.apply_procs_count(15);
        assert_eq!(s.procs_count, 15);
        s.apply_procs_count(100);
        assert_eq!(s.procs_count, 30);

        // Clicking Name selects ascending (A->Z); clicking Name again flips to Z->A.
        s.apply_click_proc_sort(ProcSort::Name);
        assert_eq!(s.proc_sort, ProcSort::Name);
        assert!(s.proc_sort_asc);
        s.apply_click_proc_sort(ProcSort::Name);
        assert!(!s.proc_sort_asc);

        // Clicking Gpu selects descending; clicking Gpu again flips to ascending.
        s.apply_click_proc_sort(ProcSort::Gpu);
        assert_eq!(s.proc_sort, ProcSort::Gpu);
        assert!(!s.proc_sort_asc);
        s.apply_click_proc_sort(ProcSort::Gpu);
        assert!(s.proc_sort_asc);

        // Round-trip ids for all ProcSort variants.
        for variant in ProcSort::ALL {
            assert_eq!(ProcSort::from_id(variant.id()), Some(variant));
        }
        for variant in CardMode::ALL {
            assert_eq!(CardMode::from_id(variant.id()), Some(variant));
        }
    }
}
