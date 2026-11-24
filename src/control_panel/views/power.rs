// SPDX-License-Identifier: GPL-3.0-only

//! Energia & Bateria: plano ativo e "Editando: [plano]" no topo, grupos do plano em edição
//! (Tela e bloqueio; CPU, GPU e Plataforma no avançado), bateria, periféricos e o estado do
//! kernel e das regras. Cada controle grava pelas mesmas `Message` de sempre; quem aplica no
//! hardware é a barra "Aplicar" da moldura.

mod advanced;

use std::sync::LazyLock;

use cosmic::{
    Element,
    iced::{self, Alignment, Length},
    theme,
    widget::{self, button},
};

use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;
use crate::central::{
    services::Battery,
    tiles::{chip_class, icon},
};
use crate::control_panel::{
    ControlPanel, Message,
    power_config::{PowerProfileKind, ProfileTuning},
};

/// Títulos dos grupos por plano, na ordem de `PowerProfileKind::ALL`. O prefixo é o `group` de
/// `search_index::INDEX`; o teste `titulos_por_plano_seguem_indice_e_rotulos` garante isso.
const TITLES_SCREEN: [&str; 3] = [
    "Tela e bloqueio · Economia",
    "Tela e bloqueio · Equilibrado",
    "Tela e bloqueio · Desempenho",
];
const TITLES_CPU: [&str; 3] = ["CPU · Economia", "CPU · Equilibrado", "CPU · Desempenho"];
const TITLES_GPU: [&str; 3] = ["GPU · Economia", "GPU · Equilibrado", "GPU · Desempenho"];
const TITLES_PLATFORM: [&str; 3] = [
    "Plataforma · Economia",
    "Plataforma · Equilibrado",
    "Plataforma · Desempenho",
];

/// Posição do plano em `PowerProfileKind::ALL` (índice dos quadros de títulos acima).
fn kind_index(kind: PowerProfileKind) -> usize {
    PowerProfileKind::ALL
        .iter()
        .position(|&k| k == kind)
        .unwrap_or(0)
}

/// Tempos de ociosidade oferecidos (minutos; 0 = nunca), os mesmos de antes.
const SCREEN_OFF_MINS: &[u32] = &[0, 2, 5, 10, 15, 30, 60];
const LOCK_MINS: &[u32] = &[0, 1, 3, 5, 10, 15, 30, 60];
const SUSPEND_AC_MINS: &[u32] = &[0, 15, 30, 45, 60, 120];
const SUSPEND_BATTERY_MINS: &[u32] = &[0, 5, 10, 15, 30, 60];

/// Taxas de atualização oferecidas `(valor gravado, rótulo)`; vazio = não alterar.
const HZ_OPTIONS: &[(&str, &str)] = &[
    ("", "Manter"),
    ("59.987", "60 Hz"),
    ("120", "120 Hz"),
    ("144", "144 Hz"),
    ("159.962", "160 Hz"),
];

fn mins_labels(values: &[u32]) -> Vec<String> {
    values.iter().map(|&m| ProfileTuning::format_mins(m)).collect()
}

static PLAN_OPTIONS: LazyLock<Vec<String>> = LazyLock::new(|| {
    PowerProfileKind::ALL
        .iter()
        .map(|k| k.label().to_owned())
        .collect()
});
static SCREEN_OFF_OPTIONS: LazyLock<Vec<String>> = LazyLock::new(|| mins_labels(SCREEN_OFF_MINS));
static LOCK_OPTIONS: LazyLock<Vec<String>> = LazyLock::new(|| mins_labels(LOCK_MINS));
static SUSPEND_AC_OPTIONS: LazyLock<Vec<String>> = LazyLock::new(|| mins_labels(SUSPEND_AC_MINS));
static SUSPEND_BATTERY_OPTIONS: LazyLock<Vec<String>> =
    LazyLock::new(|| mins_labels(SUSPEND_BATTERY_MINS));
static HZ_LABELS: LazyLock<Vec<String>> =
    LazyLock::new(|| HZ_OPTIONS.iter().map(|&(_, l)| l.to_owned()).collect());

/// Dropdown de minutos: marca o valor atual (se estiver na lista) e grava com `make`.
fn minutes_select<'a>(
    label: &'a str,
    desc: Option<&'a str>,
    options: &'a [String],
    values: &'static [u32],
    current: u32,
    make: fn(u32) -> Message,
) -> Element<'a, Message> {
    row::select(
        label,
        desc,
        options,
        values.iter().position(|&m| m == current),
        move |i| make(values[i]),
    )
}

/// Os três cartões de perfil do plano ativo (clique = `SetPowerProfile`, como sempre).
fn profile_cards(app: &ControlPanel) -> Option<Element<'_, Message>> {
    let power = app.power.as_ref()?;
    let spacing = theme::spacing();
    let defs: [(&str, &str, &str, &str, Tone); 3] = [
        (
            "power-saver",
            "power-profile-power-saver-symbolic",
            "Economia",
            "Menos consumo, ventoinhas calmas.",
            Tone::Ok,
        ),
        (
            "balanced",
            "power-profile-balanced-symbolic",
            "Equilibrado",
            "Bom para o dia a dia.",
            Tone::Warn,
        ),
        (
            "performance",
            "power-profile-performance-symbolic",
            "Desempenho",
            "Máximo de CPU e GPU, para jogos.",
            Tone::Bad,
        ),
    ];
    let mut cards = widget::row::with_capacity(defs.len()).spacing(spacing.space_xs);
    for (id, ico, label, desc, tone) in defs {
        if !power.available.iter().any(|p| p == id) {
            continue;
        }
        let head = widget::row::with_children(vec![
            widget::icon::icon(icon(ico))
                .size(18)
                .class(theme::Svg::Custom(std::rc::Rc::new(move |theme| {
                    iced::widget::svg::Style {
                        color: Some(row::tone_colors(theme, tone).1),
                    }
                })))
                .into(),
            widget::text::body(label).into(),
        ])
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);
        let body = widget::column::with_children(vec![head.into(), widget::text::caption(desc).into()])
            .spacing(spacing.space_xxxs)
            .width(Length::Fill);
        cards = cards.push(
            button::custom(body)
                .padding(spacing.space_s)
                .width(Length::Fill)
                .class(chip_class(power.active == id))
                .on_press(Message::SetPowerProfile(id.to_owned())),
        );
    }
    Some(
        widget::container(cards)
            .padding([spacing.space_s, spacing.space_m])
            .width(Length::Fill)
            .into(),
    )
}

/// Grupo "Plano ativo": cartões de perfil, estado do daemon e o seletor "Editando".
fn active_plan_group(app: &ControlPanel) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    if let Some(cards) = profile_cards(app) {
        rows.push(cards);
    }
    let daemon_up = app.power.is_some();
    let start = (!daemon_up).then(|| {
        action_button(
            "Iniciar",
            Message::RunAction(
                "Iniciar power-profiles-daemon",
                "systemctl start power-profiles-daemon.service".to_owned(),
                true,
            ),
            BtnStyle::Normal,
            !app.busy,
        )
    });
    rows.push(row::status(
        "power-profiles-daemon",
        Some("Serviço que troca o plano no sistema."),
        if daemon_up { "Ativo" } else { "Inativo" },
        if daemon_up { Tone::Ok } else { Tone::Warn },
        start.into_iter().collect(),
    ));
    let editing = app.editing_profile;
    rows.push(row::select(
        "Editando",
        Some(
            "Os grupos abaixo mostram e gravam este plano. Se ele for o plano ativo, aplica também; se não, fica guardado até você ativá-lo.",
        ),
        &PLAN_OPTIONS,
        Some(kind_index(editing)),
        |i| Message::PowerEditProfile(PowerProfileKind::ALL[i]),
    ));
    row::group("Plano ativo", None, false, rows)
}

/// Grupo "Tela e bloqueio · <plano>": os 4 tempos e a taxa de atualização do plano em edição.
fn screen_group<'a>(kind: PowerProfileKind, t: &'a ProfileTuning) -> Element<'a, Message> {
    let hz_selected = HZ_OPTIONS.iter().position(|&(val, _)| val == t.display_hz);
    let rows: Vec<Element<'a, Message>> = vec![
        minutes_select(
            "Desligar a tela",
            None,
            &SCREEN_OFF_OPTIONS,
            SCREEN_OFF_MINS,
            t.screen_off_mins,
            Message::PowerScreenOffMins,
        ),
        minutes_select(
            "Pedir senha (Tela de Login)",
            None,
            &LOCK_OPTIONS,
            LOCK_MINS,
            t.lock_mins,
            Message::PowerLockMins,
        ),
        minutes_select(
            "Suspender na tomada",
            None,
            &SUSPEND_AC_OPTIONS,
            SUSPEND_AC_MINS,
            t.suspend_ac_mins,
            Message::PowerSuspendAcMins,
        ),
        minutes_select(
            "Suspender na bateria",
            Some("Só em notebooks."),
            &SUSPEND_BATTERY_OPTIONS,
            SUSPEND_BATTERY_MINS,
            t.suspend_battery_mins,
            Message::PowerSuspendBatteryMins,
        ),
        row::select(
            "Taxa de atualização (Hz)",
            Some("Escolha um modo do monitor ou deixe sem alterar."),
            &HZ_LABELS,
            hz_selected,
            |i| Message::PowerDisplayHz(HZ_OPTIONS[i].0.to_owned()),
        ),
        row::input(
            "Valor exato da taxa",
            Some("Vazio = não alterar."),
            "Ex.: 59.987 ou 159.962",
            &t.display_hz,
            Message::PowerDisplayHz,
            None,
        ),
    ];
    row::group(
        TITLES_SCREEN[kind_index(kind)],
        Some("Vale para o plano em edição e sincroniza com o COSMIC Idle quando ele é o plano ativo."),
        false,
        rows,
    )
}

/// Texto do estado de carga ("Carregando (+12.0 W)", "Na bateria (-6.5 W)"…).
fn charge_status(bat: &Battery) -> String {
    let status_str = match bat.state {
        1 => "Carregando",
        2 => "Na bateria",
        4 => "Carregada",
        5 | 6 => "Na tomada (pausado)",
        _ if bat.charging => "Carregando",
        _ => "Na bateria",
    };
    match bat.energy_rate_w {
        Some(w) if bat.state == 1 => format!("{status_str} (+{w:.1} W)"),
        Some(w) if bat.state == 2 => format!("{status_str} (-{w:.1} W)"),
        Some(w) => format!("{status_str} ({w:.1} W)"),
        None => status_str.to_owned(),
    }
}

/// Grupo "Bateria": telemetria, modo conservação Lenovo e porcentagem na barra.
fn battery_group<'a>(app: &'a ControlPanel, bat: &'a Battery) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = vec![
        row::kv("Carga", format!("{:.0}%", bat.percentage)),
        row::kv("Status", charge_status(bat)),
    ];
    if let Some(charger) = bat.charger.as_ref().filter(|c| c.online) {
        let mut parts = Vec::new();
        if let Some(v) = charger.voltage_v.or(charger.max_voltage_v) {
            parts.push(format!("{v:.1} V"));
        }
        if let Some(a) = charger.current_a {
            parts.push(format!("{a:.2} A"));
        }
        if let Some(w) = charger.power_w {
            parts.push(format!("{w:.1} W"));
        } else if let Some(max_w) = charger.max_power_w {
            parts.push(format!("máx. {max_w:.0} W"));
        }
        if !parts.is_empty() {
            rows.push(row::kv("Entrada (USB-C PD / AC)", parts.join(" · ")));
        }
    }
    if let Some(v) = bat.voltage_v {
        rows.push(row::kv("Tensão da bateria", format!("{v:.2} V")));
    }
    let mut cap = Vec::new();
    if let (Some(now), Some(full)) = (bat.energy_wh, bat.energy_full_wh) {
        cap.push(format!("{now:.1} / {full:.1} Wh"));
    }
    if let Some(h) = bat.health_pct {
        cap.push(format!("Saúde {h:.0}%"));
    }
    if let Some(c) = bat.cycles {
        cap.push(format!("{c} ciclos"));
    }
    if !cap.is_empty() {
        rows.push(row::kv("Capacidade e saúde", cap.join(" · ")));
    }
    if let Some(cons) = bat.conservation_mode {
        rows.push(row::toggle(
            "Modo conservação Lenovo",
            Some("Limita a carga em 80% para preservar a bateria."),
            cons,
            |_| Message::ToggleConservationMode,
        ));
    }
    rows.push(row::toggle(
        "Mostrar porcentagem da bateria na barra",
        None,
        app.show_percentage,
        |_| Message::ToggleShowPercentage,
    ));
    row::group("Bateria", None, false, rows)
}

/// Grupo "Periféricos": bateria dos dispositivos sem fio conectados.
fn peripherals_group(app: &ControlPanel) -> Element<'_, Message> {
    let mut rows: Vec<Element<'_, Message>> = app
        .peripherals
        .iter()
        .map(|p| row::list_item(&p.name, format!("Bateria {:.0}%", p.percentage), None, None))
        .collect();
    if rows.is_empty() {
        rows.push(row::note("Nenhum periférico sem fio conectado."));
    }
    row::group("Periféricos", None, false, rows)
}

/// Grupo "Kernel, módulos e regras" (avançado): estado das regras e o instalador.
fn kernel_group(app: &ControlPanel) -> Element<'_, Message> {
    let snap = &app.snapshot;
    let repo_dir = snap
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());
    let install_cmd = format!("\"{repo_dir}/extras/power/install-economia-energia.sh\"");
    let rule = |label: &'static str, desc: Option<&'static str>, ok: bool, ok_pill: &'static str, off_pill: &'static str| {
        row::status(
            label,
            desc,
            if ok { ok_pill } else { off_pill },
            if ok { Tone::Ok } else { Tone::Warn },
            Vec::new(),
        )
    };
    let install_style = if snap.economia_energia_active {
        BtnStyle::Normal
    } else {
        BtnStyle::Primary
    };
    let rows: Vec<Element<'_, Message>> = vec![
        rule("Daemon economia-energia", Some("economia-energia.service"), snap.economia_energia_active, "Ativo", "Inativo"),
        rule(
            "Áudio power_save no boot",
            Some("/etc/modprobe.d/99-economia-energia.conf"),
            snap.audio_modprobe_configured,
            "Configurado",
            "Ausente",
        ),
        rule(
            "Leitura RAPL em watts",
            Some("/etc/udev/rules.d/99-rapl.rules: libera a leitura de consumo da CPU."),
            snap.rapl_udev_configured,
            "Ativo",
            "Ausente",
        ),
        rule(
            "Módulo i2c-dev",
            Some("Brilho DDC/CI nos monitores."),
            snap.i2c_dev_module_configured,
            "No boot",
            "Ausente",
        ),
        rule(
            "Módulo nct6775",
            Some("Sensores e ventoinhas da placa-mãe."),
            snap.nct6775_module_configured,
            "No boot",
            "Ausente",
        ),
        row::buttons(
            "Instalar ou atualizar regras",
            Some("Pede senha de administrador."),
            vec![action_button(
                "Instalar / Atualizar",
                Message::RunAction("Instalar economia-energia e regras", install_cmd, true),
                install_style,
                !app.busy,
            )],
        ),
    ];
    row::group("Kernel, módulos e regras", None, true, rows)
}

pub(super) fn power_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = theme::spacing();
    let editing = app.editing_profile;
    let t = app.power_config.get(editing);

    let mut items: Vec<Element<'_, Message>> = vec![
        active_plan_group(app),
        screen_group(editing, t),
    ];
    if let Some(bat) = &app.battery {
        items.push(battery_group(app, bat));
    }
    items.push(peripherals_group(app));

    // CPU, GPU, Plataforma e Kernel ficam atrás de "Mostrar ajustes avançados" (moldura).
    if app.advanced_open {
        items.push(advanced::cpu_group(app, editing, t));
        items.push(advanced::gpu_group(app, editing, t));
        items.push(advanced::platform_group(editing, t));
        items.push(kernel_group(app));
    }

    widget::column::with_children(items)
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página (o "Aplicar" vive na barra do rodapé da moldura).
pub(super) fn power_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    vec![action_button(
        "Restaurar padrões",
        Message::PowerResetDefaults,
        BtnStyle::Normal,
        !app.busy,
    )]
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn power_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_panel::{Section, chrome};

    #[test]
    fn titulos_por_plano_seguem_indice_e_rotulos() {
        let grupos: Vec<&str> = chrome::section_groups(Section::Power)
            .into_iter()
            .map(|(g, _)| g)
            .collect();
        let tabelas: [(&str, &[&str; 3]); 4] = [
            ("Tela e bloqueio", &TITLES_SCREEN),
            ("CPU", &TITLES_CPU),
            ("GPU", &TITLES_GPU),
            ("Plataforma", &TITLES_PLATFORM),
        ];
        for (base, titulos) in tabelas {
            assert!(grupos.contains(&base), "{base} não está no índice de busca");
            for kind in PowerProfileKind::ALL {
                assert_eq!(
                    titulos[kind_index(kind)],
                    format!("{base} · {}", kind.label()),
                    "título de {base} para {kind:?}"
                );
            }
        }
    }

    #[test]
    fn grupos_fixos_existem_no_indice() {
        let grupos: Vec<&str> = chrome::section_groups(Section::Power)
            .into_iter()
            .map(|(g, _)| g)
            .collect();
        for g in ["Plano ativo", "Periféricos", "Kernel, módulos e regras"] {
            assert!(grupos.contains(&g), "{g} não está no índice de busca");
        }
    }

    #[test]
    fn opcoes_de_tempo_preservam_os_valores_de_antes() {
        assert_eq!(SCREEN_OFF_OPTIONS.len(), SCREEN_OFF_MINS.len());
        assert_eq!(SCREEN_OFF_OPTIONS[0], "Nunca");
        assert_eq!(LOCK_OPTIONS.last().map(String::as_str), Some("1 h"));
        assert_eq!(SUSPEND_AC_OPTIONS.len(), 6);
        assert_eq!(SUSPEND_BATTERY_OPTIONS.len(), 6);
        assert_eq!(HZ_LABELS.len(), HZ_OPTIONS.len());
        assert_eq!(HZ_OPTIONS[0].0, "");
    }
}
