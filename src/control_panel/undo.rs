// SPDX-License-Identifier: GPL-3.0-only

//! "Alterados recentemente" e "Desfazer" (spec §7.6): descreve, antes de a `Message` ser aplicada,
//! uma mudança simples de valor que grava em arquivo e devolve a `Message` que a desfaz (a mesma,
//! com o valor anterior). Restaurar padrões e ações com efeito no sistema (plano de energia,
//! monitor principal, espaçamento, segurança) não passam por aqui e não têm "Desfazer".

use std::time::Duration;

use super::{ControlPanel, Message, Section, feedback::RecentChange};

/// Separador entre o rótulo e o valor novo em `RecentChange::label`.
const ARROW: &str = " → ";

/// Uma mudança que pode ser desfeita.
#[derive(Clone, Debug)]
pub struct Described {
    pub label: String,
    pub value: String,
    pub section: Section,
    /// A `Message` que volta ao valor anterior.
    pub undo: Message,
    /// Rajadas do mesmo ajuste (arrastar um slider, trocar o dropdown várias vezes) viram uma
    /// entrada só, que desfaz até o valor de antes da rajada. Interruptores não juntam.
    pub coalesce: bool,
}

fn onoff(on: bool) -> String {
    if on { "Ligado" } else { "Desligado" }.to_owned()
}

fn mins(n: u32) -> String {
    if n == 0 { "Nunca".to_owned() } else { format!("{n} min") }
}

fn mhz(n: u32, zero: &str) -> String {
    if n == 0 { zero.to_owned() } else { format!("{n} MHz") }
}

fn watts(n: u32) -> String {
    if n == 0 { "Padrão".to_owned() } else { format!("{n} W") }
}

fn toggle(label: &str, section: Section, after: bool, same: &Message) -> Option<Described> {
    Some(Described {
        label: label.to_owned(),
        value: onoff(after),
        section,
        undo: same.clone(),
        coalesce: false,
    })
}

/// Mudança de valor; `None` quando o valor formatado não mudou.
fn value(
    label: impl Into<String>,
    section: Section,
    old: String,
    new: String,
    undo: Message,
) -> Option<Described> {
    (old != new).then(|| Described {
        label: label.into(),
        value: new,
        section,
        undo,
        coalesce: true,
    })
}

/// Descreve `msg` contra o estado de `app` **antes** de ela ser aplicada.
pub fn describe(app: &ControlPanel, msg: &Message) -> Option<Described> {
    describe_power(app, msg)
        .or_else(|| describe_appearance(app, msg))
        .or_else(|| describe_capture_login(app, msg))
}

fn describe_power(app: &ControlPanel, msg: &Message) -> Option<Described> {
    let kind = app.editing_profile;
    let t = app.power_config.get(kind);
    let plan = kind.label();
    let sec = Section::Power;
    let wrap = |m: Message| Message::UndoAsProfile(kind, Box::new(m));
    let label = |name: &str| format!("{name} · {plan}");
    let tog = |name: &str, after: bool| {
        toggle(&label(name), sec, after, msg).map(|mut d| {
            d.undo = wrap(msg.clone());
            d
        })
    };
    match msg {
        Message::PowerToggleBoost => tog("Boost / Turbo", !t.boost),
        Message::PowerToggleDisableCcd1 => tog("Desligar CCD1", !t.disable_ccd1),
        Message::PowerToggleNvidiaPersistence => tog("Persistence Mode", !t.nvidia_persistence),
        Message::PowerToggleAudioSave => tog("Economia de energia do áudio", !t.audio_power_save),
        Message::PowerToggleWifiSave => tog("Economia de energia do Wi-Fi", !t.wifi_power_save),
        Message::PowerCpuMaxMhz(n) => value(
            label("Teto de frequência"),
            sec,
            mhz(t.cpu_max_mhz, "Máximo do hardware"),
            mhz(*n, "Máximo do hardware"),
            wrap(Message::PowerCpuMaxMhz(t.cpu_max_mhz)),
        ),
        Message::PowerEpp(v) => value(
            label("Preferência de eficiência (EPP)"),
            sec,
            t.epp.clone(),
            v.clone(),
            wrap(Message::PowerEpp(t.epp.clone())),
        ),
        Message::PowerIntelMaxPerf(p) => value(
            label("Desempenho máximo (Intel)"),
            sec,
            format!("{}%", t.intel_max_perf_pct),
            format!("{}%", p.clamp(&10, &100)),
            wrap(Message::PowerIntelMaxPerf(t.intel_max_perf_pct)),
        ),
        Message::PowerRaplPl1(w) => value(
            label("Limite sustentado (RAPL PL1)"),
            sec,
            watts(t.rapl_pl1_w),
            watts(*w),
            wrap(Message::PowerRaplPl1(t.rapl_pl1_w)),
        ),
        Message::PowerRaplPl2(w) => value(
            label("Pico curto (RAPL PL2)"),
            sec,
            watts(t.rapl_pl2_w),
            watts(*w),
            wrap(Message::PowerRaplPl2(t.rapl_pl2_w)),
        ),
        Message::PowerMaxOnlineCores(n) => {
            let cores = |c: u32| if c == 0 { "Todos".to_owned() } else { c.to_string() };
            value(
                label("Núcleos online"),
                sec,
                cores(t.max_online_cores),
                cores(*n),
                wrap(Message::PowerMaxOnlineCores(t.max_online_cores)),
            )
        }
        Message::PowerNvidiaPowerLimit(w) => value(
            label("Limite de potência da GPU"),
            sec,
            watts(t.nvidia_power_limit_w),
            watts(*w),
            wrap(Message::PowerNvidiaPowerLimit(t.nvidia_power_limit_w)),
        ),
        Message::PowerNvidiaGpuClock(n) => value(
            label("Clock da GPU"),
            sec,
            mhz(t.nvidia_gpu_clock_mhz, "Livre"),
            mhz(*n, "Livre"),
            wrap(Message::PowerNvidiaGpuClock(t.nvidia_gpu_clock_mhz)),
        ),
        Message::PowerNvidiaMemClock(n) => value(
            label("Clock da memória da GPU"),
            sec,
            mhz(t.nvidia_mem_clock_mhz, "Livre"),
            mhz(*n, "Livre"),
            wrap(Message::PowerNvidiaMemClock(t.nvidia_mem_clock_mhz)),
        ),
        Message::PowerIntelIgpuMaxMhz(n) => value(
            label("Teto da GPU integrada"),
            sec,
            mhz(t.intel_igpu_max_mhz, "Máximo do hardware"),
            mhz(*n, "Máximo do hardware"),
            wrap(Message::PowerIntelIgpuMaxMhz(t.intel_igpu_max_mhz)),
        ),
        Message::PowerDisplayHz(hz) => {
            let shown = |h: &str| if h.is_empty() { "Inalterada".to_owned() } else { format!("{h} Hz") };
            value(
                label("Taxa de atualização"),
                sec,
                shown(&t.display_hz),
                shown(hz),
                wrap(Message::PowerDisplayHz(t.display_hz.clone())),
            )
        }
        Message::PowerScreenOffMins(n) => value(
            label("Desligar a tela"),
            sec,
            mins(t.screen_off_mins),
            mins(*n),
            wrap(Message::PowerScreenOffMins(t.screen_off_mins)),
        ),
        Message::PowerLockMins(n) => value(
            label("Pedir senha"),
            sec,
            mins(t.lock_mins),
            mins(*n),
            wrap(Message::PowerLockMins(t.lock_mins)),
        ),
        Message::PowerSuspendAcMins(n) => value(
            label("Suspender na tomada"),
            sec,
            mins(t.suspend_ac_mins),
            mins(*n),
            wrap(Message::PowerSuspendAcMins(t.suspend_ac_mins)),
        ),
        Message::PowerSuspendBatteryMins(n) => value(
            label("Suspender na bateria"),
            sec,
            mins(t.suspend_battery_mins),
            mins(*n),
            wrap(Message::PowerSuspendBatteryMins(t.suspend_battery_mins)),
        ),
        Message::PowerPcieAspm(v) => value(
            label("PCIe ASPM"),
            sec,
            t.pcie_aspm.clone(),
            v.clone(),
            wrap(Message::PowerPcieAspm(t.pcie_aspm.clone())),
        ),
        Message::PowerSataAlpm(v) => value(
            label("SATA ALPM"),
            sec,
            t.sata_alpm.clone(),
            v.clone(),
            wrap(Message::PowerSataAlpm(t.sata_alpm.clone())),
        ),
        _ => None,
    }
}

fn describe_appearance(app: &ControlPanel, msg: &Message) -> Option<Described> {
    let n = &app.notif_config;
    let nsec = Section::Notifications;
    let osd = &app.osd_config;
    let mon = &app.monitor_settings;
    match msg {
        // Central
        Message::CentralToggleDarkMode => toggle("Modo escuro", Section::Central, !app.dark_mode, msg),
        Message::CentralToggleDnd => toggle("Não perturbe", Section::Central, !app.do_not_disturb, msg),
        Message::CentralToggleNewWorkspacesTile => toggle(
            "Novas áreas de trabalho em mosaico",
            Section::Central,
            !app.tiling_config.new_workspaces_tile,
            msg,
        ),
        Message::CentralToggleActiveHint => toggle(
            "Destaque da janela ativa",
            Section::Central,
            !app.tiling_config.active_hint,
            msg,
        ),
        Message::CentralToggleHotEdge => toggle("Hot Edge", Section::Central, !app.hot_edge, msg),
        // Monitor (esquerda)
        Message::MonitorToggleBar(item) => toggle(
            &format!("{} na barra", item.name()),
            Section::Monitor,
            !mon.shows_on_bar(*item),
            msg,
        ),
        Message::MonitorBarMode(item, mode) => value(
            format!("{} na barra", item.name()),
            Section::Monitor,
            mon.bar_mode(*item).name().to_owned(),
            mode.name().to_owned(),
            Message::MonitorBarMode(*item, mon.bar_mode(*item)),
        ),
        Message::MonitorCardMode(panel, mode) => value(
            format!("Cartão {}", panel.name()),
            Section::Monitor,
            mon.card_mode(*panel).name().to_owned(),
            mode.name().to_owned(),
            Message::MonitorCardMode(*panel, mon.card_mode(*panel)),
        ),
        Message::MonitorWeatherMode(mode) => value(
            "Cartão de clima",
            Section::Monitor,
            mon.weather_mode().name().to_owned(),
            mode.name().to_owned(),
            Message::MonitorWeatherMode(mon.weather_mode()),
        ),
        Message::MonitorComponentPoll(comp, secs) => {
            let unit = |v: u64| {
                let suffix = if matches!(comp, super::MonitorPollComponent::Weather) { "min" } else { "s" };
                format!("{v} {suffix}")
            };
            value(
                format!("Atualização · {}", comp.name()),
                Section::Monitor,
                unit(mon.component_poll(*comp)),
                unit(*secs),
                Message::MonitorComponentPoll(*comp, mon.component_poll(*comp)),
            )
        }
        // Notificações
        Message::NotifTimeout(secs) => {
            let shown = |v: u32| if v == 0 { "Sem limite".to_owned() } else { format!("{v} s") };
            value(
                "Tempo na tela",
                nsec,
                shown(n.timeout_secs),
                shown(*secs),
                Message::NotifTimeout(n.timeout_secs),
            )
        }
        Message::NotifFormat(f) => value(
            "Formato do aviso",
            nsec,
            n.format.label().to_owned(),
            f.label().to_owned(),
            Message::NotifFormat(n.format),
        ),
        Message::NotifPosition(p) => value(
            "Posição do aviso",
            nsec,
            n.position.label().to_owned(),
            p.label().to_owned(),
            Message::NotifPosition(n.position),
        ),
        Message::NotifOpacity(v) => value(
            "Opacidade do aviso",
            nsec,
            format!("{:.0}%", n.opacity * 100.0),
            format!("{:.0}%", v * 100.0),
            Message::NotifOpacity(n.opacity),
        ),
        Message::NotifCornerRadius(v) => value(
            "Arredondamento do aviso",
            nsec,
            format!("{:.0} px", n.corner_radius),
            format!("{:.0} px", v),
            Message::NotifCornerRadius(n.corner_radius),
        ),
        Message::NotifMaxBodyLines(v) => value(
            "Linhas do corpo do aviso",
            nsec,
            n.max_body_lines.to_string(),
            v.to_string(),
            Message::NotifMaxBodyLines(n.max_body_lines),
        ),
        Message::NotifToggleShowBody => toggle("Mostrar o corpo do aviso", nsec, !n.show_body, msg),
        Message::NotifToggleShowActions => toggle("Mostrar ações do aviso", nsec, !n.show_actions, msg),
        Message::NotifToggleGroupByApp => toggle("Agrupar por aplicativo", nsec, !n.group_by_app, msg),
        // OSD de dispositivos
        Message::OsdDurationMs(ms) => value(
            "Duração do OSD",
            Section::DevicesOsd,
            format!("{} ms", osd.duration.as_millis()),
            format!("{ms} ms"),
            Message::OsdDurationMs(osd.duration.as_millis() as u64),
        ),
        Message::OsdBatteryCritical(p) => value(
            "Bateria crítica (OSD)",
            Section::DevicesOsd,
            format!("{}%", osd.battery_critical),
            format!("{p}%"),
            Message::OsdBatteryCritical(osd.battery_critical),
        ),
        Message::OsdBatteryIntervalMins(m) => {
            let old = osd.legacy_battery_interval.as_secs() / 60;
            value(
                "Intervalo do aviso de bateria",
                Section::DevicesOsd,
                format!("{old} min"),
                format!("{m} min"),
                Message::OsdBatteryIntervalMins(old),
            )
        }
        _ => None,
    }
}

fn describe_capture_login(app: &ControlPanel, msg: &Message) -> Option<Described> {
    let s = &app.screenshot_config;
    let cap = Section::Capture;
    let g = &app.greeter;
    let login = Section::Login;
    match msg {
        // Captura de tela
        Message::ScreenshotToggleCopy => {
            toggle("Copiar para a área de transferência", cap, !s.copy_clipboard, msg)
        }
        Message::ScreenshotToggleSound => toggle("Som da captura", cap, !s.play_sound, msg),
        Message::ScreenshotOpenMode(mode) => value(
            "Depois de capturar",
            cap,
            s.open_mode.label().to_owned(),
            mode.label().to_owned(),
            Message::ScreenshotOpenMode(s.open_mode),
        ),
        Message::ScreenshotSattyTool(tool) => value(
            "Ferramenta inicial do Satty",
            cap,
            s.satty_tool.clone(),
            tool.clone(),
            Message::ScreenshotSattyTool(s.satty_tool.clone()),
        ),
        Message::ScreenshotBorderWidth(w) => value(
            "Espessura da borda",
            cap,
            format!("{} px", s.border_width),
            format!("{} px", w.clamp(&1, &6)),
            Message::ScreenshotBorderWidth(s.border_width),
        ),
        Message::ScreenshotOverlayOpacity(p) => value(
            "Escurecimento do fundo",
            cap,
            format!("{}%", s.overlay_opacity),
            format!("{}%", p.clamp(&0, &85)),
            Message::ScreenshotOverlayOpacity(s.overlay_opacity),
        ),
        Message::ScreenshotTimeoutSecs(t) => value(
            "Tempo da notificação",
            cap,
            format!("{} s", s.notify_timeout_secs),
            format!("{} s", t.clamp(&30, &3600)),
            Message::ScreenshotTimeoutSecs(s.notify_timeout_secs),
        ),
        // Tela de login
        Message::GreeterToggleShuffle => toggle("Fundos em ordem aleatória", login, !g.wallpaper_shuffle, msg),
        Message::GreeterToggleClock24h => toggle("Relógio em 24 h", login, !g.clock_24h, msg),
        Message::GreeterToggleShowSeconds => toggle("Mostrar segundos", login, !g.show_seconds, msg),
        Message::GreeterToggleShowDate => toggle("Mostrar a data", login, !g.show_date, msg),
        Message::GreeterToggleShowAvatar => toggle("Mostrar o avatar", login, !g.show_avatar, msg),
        Message::GreeterToggleShowUsername => toggle("Mostrar o usuário", login, !g.show_username, msg),
        Message::GreeterToggleIsolatedBinary => {
            toggle("Binário isolado do login", login, !g.isolated_binary, msg)
        }
        Message::GreeterStyle(v) => text_value("Estilo do login", login, &g.style, v, Message::GreeterStyle),
        Message::GreeterWallpaperMode(v) => {
            text_value("Modo do fundo", login, &g.wallpaper_mode, v, Message::GreeterWallpaperMode)
        }
        Message::GreeterTheme(v) => text_value("Tema do login", login, &g.theme, v, Message::GreeterTheme),
        Message::GreeterClockSize(v) => {
            text_value("Tamanho do relógio", login, &g.clock_size, v, Message::GreeterClockSize)
        }
        Message::GreeterFontFamily(v) => {
            text_value("Fonte do login", login, &g.font_family, v, Message::GreeterFontFamily)
        }
        Message::GreeterFontWeight(v) => {
            text_value("Peso da fonte", login, &g.font_weight, v, Message::GreeterFontWeight)
        }
        Message::GreeterIntervalMins(m) => value(
            "Troca de fundo",
            login,
            format!("{} min", g.wallpaper_interval_mins),
            format!("{} min", m.clamp(&1, &1440)),
            Message::GreeterIntervalMins(g.wallpaper_interval_mins),
        ),
        Message::GreeterBlurRadius(r) => value(
            "Desfoque do fundo",
            login,
            g.blur_radius.to_string(),
            r.min(&60).to_string(),
            Message::GreeterBlurRadius(g.blur_radius),
        ),
        Message::GreeterDimOpacity(v) => value(
            "Escurecimento do fundo",
            login,
            format!("{:.0}%", g.dim_opacity * 100.0),
            format!("{:.0}%", v.clamp(0.0, 0.85) * 100.0),
            Message::GreeterDimOpacity(g.dim_opacity),
        ),
        Message::GreeterCardOpacity(v) => value(
            "Opacidade do cartão",
            login,
            format!("{:.0}%", g.card_opacity * 100.0),
            format!("{:.0}%", v.clamp(0.0, 0.95) * 100.0),
            Message::GreeterCardOpacity(g.card_opacity),
        ),
        Message::GreeterCardRadius(v) => value(
            "Arredondamento do cartão",
            login,
            format!("{:.0} px", g.card_radius),
            format!("{:.0} px", v.clamp(4.0, 32.0)),
            Message::GreeterCardRadius(g.card_radius),
        ),
        _ => None,
    }
}

fn text_value(
    label: &str,
    section: Section,
    old: &str,
    new: &str,
    make: fn(String) -> Message,
) -> Option<Described> {
    value(label, section, old.to_owned(), new.to_owned(), make(old.to_owned()))
}

/// Parte do rótulo antes do valor novo: identifica "o mesmo ajuste".
fn key(label: &str) -> &str {
    label.split(ARROW).next().unwrap_or(label)
}

/// Insere `new` na lista de recentes (mais recente primeiro). Uma rajada do mesmo ajuste dentro
/// de `window` vira uma entrada só: mostra o valor mais novo e guarda o "Desfazer" da primeira,
/// que volta ao valor de antes da rajada. Sem "Desfazer" no topo (já usado), não junta.
pub fn coalesce(
    mut items: Vec<RecentChange<Message>>,
    new: RecentChange<Message>,
    can_merge: bool,
    window: Duration,
) -> Vec<RecentChange<Message>> {
    // A `Message` de desfazer de um interruptor é a mesma de alternar: uma entrada antiga
    // inverteria o estado ATUAL. Mudança nova do mesmo ajuste invalida o "Desfazer" das antigas.
    let merges = can_merge
        && items.first().is_some_and(|head| {
            head.undo.is_some()
                && head.section == new.section
                && key(&head.label) == key(&new.label)
                && new.at.saturating_duration_since(head.at) <= window
        });
    for (i, old) in items.iter_mut().enumerate() {
        if (!merges || i > 0) && old.section == new.section && key(&old.label) == key(&new.label) {
            old.undo = None;
        }
    }
    if can_merge
        && let Some(head) = items.first()
        && head.undo.is_some()
        && head.section == new.section
        && key(&head.label) == key(&new.label)
        && new.at.saturating_duration_since(head.at) <= window
    {
        let kept = head.undo.clone();
        items[0] = RecentChange { undo: kept, ..new };
        return items;
    }
    items.insert(0, new);
    items
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn change(label: &str, at: Instant, undo: Option<Message>) -> RecentChange<Message> {
        RecentChange { label: label.to_owned(), section: Section::Notifications, at, undo }
    }

    fn undo_value(c: &RecentChange<Message>) -> Option<u32> {
        match c.undo {
            Some(Message::NotifTimeout(v)) => Some(v),
            _ => None,
        }
    }

    #[test]
    fn slider_burst_keeps_one_entry_and_the_first_undo() {
        let t0 = Instant::now();
        let w = Duration::from_secs(3);
        let mut items = Vec::new();
        items = coalesce(items, change("Tempo → 5 s", t0, Some(Message::NotifTimeout(3))), true, w);
        items = coalesce(
            items,
            change("Tempo → 8 s", t0 + Duration::from_secs(1), Some(Message::NotifTimeout(5))),
            true,
            w,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "Tempo → 8 s");
        assert_eq!(undo_value(&items[0]), Some(3));
    }

    #[test]
    fn toggles_and_late_changes_do_not_merge() {
        let t0 = Instant::now();
        let w = Duration::from_secs(3);
        let items = coalesce(Vec::new(), change("Som → Ligado", t0, Some(Message::None)), false, w);
        let items = coalesce(items, change("Som → Desligado", t0, Some(Message::None)), false, w);
        assert_eq!(items.len(), 2);
        // fora da janela: entrada nova mesmo sendo o mesmo ajuste
        let items = coalesce(
            items,
            change("Som → Ligado", t0 + Duration::from_secs(10), Some(Message::None)),
            true,
            w,
        );
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn spent_undo_or_other_setting_does_not_merge() {
        let t0 = Instant::now();
        let w = Duration::from_secs(3);
        let spent = vec![change("Tempo → 5 s", t0, None)];
        let items = coalesce(spent, change("Tempo → 8 s", t0, Some(Message::NotifTimeout(5))), true, w);
        assert_eq!(items.len(), 2);
        let other = vec![change("Tempo → 5 s", t0, Some(Message::NotifTimeout(3)))];
        let items = coalesce(other, change("Opacidade → 70%", t0, Some(Message::NotifTimeout(9))), true, w);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn newer_change_of_same_toggle_drops_older_undo_only_for_that_key() {
        let t0 = Instant::now();
        let w = Duration::from_secs(3);
        let mut items = Vec::new();
        items = coalesce(items, change("Som → Ligado", t0, Some(Message::None)), false, w);
        items = coalesce(items, change("Outro → Ligado", t0, Some(Message::None)), false, w);
        items = coalesce(items, change("Som → Desligado", t0, Some(Message::None)), false, w);
        assert_eq!(items.len(), 3);
        assert!(items[0].undo.is_some(), "a mais recente mantém o undo");
        assert!(items[1].undo.is_some(), "chave diferente não é afetada");
        assert!(items[2].undo.is_none(), "a antiga do mesmo interruptor perde o undo");
    }

    #[test]
    fn key_is_the_label_before_the_arrow() {
        assert_eq!(key("Teto · Equilibrado → 3000 MHz"), "Teto · Equilibrado");
        assert_eq!(key("sem seta"), "sem seta");
    }

    #[test]
    fn value_formats_use_the_spec_words_for_zero() {
        assert_eq!(mhz(0, "Máximo do hardware"), "Máximo do hardware");
        assert_eq!(mhz(3200, "Livre"), "3200 MHz");
        assert_eq!(watts(0), "Padrão");
        assert_eq!(mins(0), "Nunca");
        assert_eq!(mins(15), "15 min");
        assert_eq!(onoff(true), "Ligado");
    }

    #[test]
    fn unchanged_value_is_not_recorded() {
        assert!(value("x", Section::Power, "5 s".into(), "5 s".into(), Message::None).is_none());
        let d = value("x", Section::Power, "5 s".into(), "8 s".into(), Message::None).unwrap();
        assert!(d.coalesce);
        assert!(toggle("x", Section::Power, true, &Message::None).is_some_and(|d| !d.coalesce));
    }
}
