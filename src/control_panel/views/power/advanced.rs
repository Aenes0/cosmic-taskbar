// SPDX-License-Identifier: GPL-3.0-only

//! Grupos avançados de Energia (CPU, GPU e Plataforma) do plano em edição. Só aparecem com
//! "Mostrar ajustes avançados" aberto; cada controle grava no plano em edição pelas mesmas
//! `Message` de antes.

use std::sync::LazyLock;

use cosmic::Element;

use super::super::row;
use super::{TITLES_CPU, TITLES_GPU, TITLES_PLATFORM, kind_index};
use crate::control_panel::{
    ControlPanel, Message,
    power_config::{self, PowerProfileKind, ProfileTuning},
};

/// Tetos de frequência oferecidos no menu `(MHz, rótulo)`; 0 = máximo do hardware.
const FREQ_PRESETS: &[(u32, &str)] = &[
    (0, "Máx."),
    (2000, "2.0 GHz"),
    (2400, "2.4 GHz"),
    (3200, "3.2 GHz"),
    (4000, "4.0 GHz"),
    (4400, "4.4 GHz"),
];

static FREQ_OPTIONS: LazyLock<Vec<String>> =
    LazyLock::new(|| FREQ_PRESETS.iter().map(|&(_, l)| l.to_owned()).collect());

static EPP_LABELS: LazyLock<Vec<String>> = LazyLock::new(|| {
    power_config::EPP_OPTIONS
        .iter()
        .map(|&(_, l)| l.to_owned())
        .collect()
});

/// Texto do selo de um slider cujo zero significa "sem limite".
fn shown_or(value: u32, zero_text: &str, unit: &str) -> String {
    if value == 0 {
        zero_text.to_owned()
    } else {
        format!("{value} {unit}")
    }
}

/// Posição do valor atual numa lista de opções crus; `usize::MAX` = nenhuma marcada.
fn marked(raw: &[(&str, &str)], current: &str) -> usize {
    raw.iter()
        .position(|&(val, _)| val == current)
        .unwrap_or(usize::MAX)
}

pub(super) fn cpu_group<'a>(
    app: &'a ControlPanel,
    kind: PowerProfileKind,
    t: &'a ProfileTuning,
) -> Element<'a, Message> {
    let hw = &app.snapshot.hardware;
    let topo = &app.cpu_topology;
    let hw_max = topo.hw_max_mhz.max(5500).max(t.cpu_max_mhz);
    let max_cores_cap = topo.physical_cores.max(16).max(t.max_online_cores);
    let topo_summary = format!(
        "{} · {} núcleos físicos / {} threads{}",
        hw.cpu_model,
        topo.physical_cores,
        topo.logical_cpus,
        if topo.is_amd_dual_ccd { " · AMD Dual-CCD" } else { "" }
    );

    let mut rows: Vec<Element<'a, Message>> = vec![
        row::kv("Processador", topo_summary),
        row::toggle(
            "Boost / Turbo",
            Some("Deixa os núcleos passarem da frequência base quando há folga térmica."),
            t.boost,
            |_| Message::PowerToggleBoost,
        ),
        row::select(
            "Teto de frequência",
            Some("Limita a frequência máxima da CPU neste plano. Máx. usa o limite do hardware."),
            &FREQ_OPTIONS,
            FREQ_PRESETS.iter().position(|&(mhz, _)| mhz == t.cpu_max_mhz),
            |i| Message::PowerCpuMaxMhz(FREQ_PRESETS[i].0),
        ),
        row::slider(
            "Ajuste fino do teto",
            Some("Passos de 100 MHz; abaixo de 800 MHz volta ao máximo do hardware."),
            0.0..=hw_max as f32,
            100.0,
            t.cpu_max_mhz as f32,
            shown_or(t.cpu_max_mhz, "Máximo do hardware", "MHz"),
            ("0".to_owned(), format!("{hw_max} MHz")),
            |v| {
                let rounded = ((v / 100.0).round() as u32) * 100;
                Message::PowerCpuMaxMhz(if rounded < 800 { 0 } else { rounded })
            },
            None,
        ),
        row::select(
            "Preferência de eficiência (EPP)",
            Some("EPP (Energy Performance Preference): quanto o processador prioriza economia sobre velocidade."),
            &EPP_LABELS,
            power_config::EPP_OPTIONS
                .iter()
                .position(|&(val, _)| val == t.epp),
            |i| Message::PowerEpp(power_config::EPP_OPTIONS[i].0.to_owned()),
        ),
    ];

    if hw.is_intel {
        rows.push(row::slider(
            "Desempenho máximo (Intel max_perf_pct)",
            Some("Percentual do desempenho máximo que o processador Intel pode usar."),
            10.0..=100.0,
            5.0,
            t.intel_max_perf_pct as f32,
            format!("{}%", t.intel_max_perf_pct),
            ("10%".to_owned(), "100%".to_owned()),
            |v| Message::PowerIntelMaxPerf(v.round() as u32),
            None,
        ));
        rows.push(row::slider(
            "Limite sustentado (RAPL PL1)",
            Some("RAPL PL1: potência que a CPU pode manter por tempo indefinido."),
            0.0..=100.0,
            1.0,
            t.rapl_pl1_w.min(100) as f32,
            shown_or(t.rapl_pl1_w, "Padrão", "W"),
            ("Padrão".to_owned(), "100 W".to_owned()),
            |v| Message::PowerRaplPl1(v.round() as u32),
            None,
        ));
        rows.push(row::slider(
            "Pico curto (RAPL PL2)",
            Some("RAPL PL2: potência permitida em rajadas de poucos segundos."),
            0.0..=120.0,
            1.0,
            t.rapl_pl2_w.min(120) as f32,
            shown_or(t.rapl_pl2_w, "Padrão", "W"),
            ("Padrão".to_owned(), "120 W".to_owned()),
            |v| Message::PowerRaplPl2(v.round() as u32),
            None,
        ));
    }

    rows.push(row::toggle(
        "Desligar CCD1",
        Some(if topo.is_amd_dual_ccd {
            "CCD1 é a segunda metade dos núcleos (AMD dual-CCD, detectado aqui). Economiza, mas perde desempenho em multitarefa."
        } else {
            "CCD1 é a segunda metade dos núcleos (AMD dual-CCD). Economiza, mas perde desempenho em multitarefa."
        }),
        t.disable_ccd1,
        |_| Message::PowerToggleDisableCcd1,
    ));
    rows.push(row::slider(
        "Núcleos online",
        Some("Mantém só N núcleos físicos (e seus threads) ligados."),
        0.0..=max_cores_cap as f32,
        1.0,
        t.max_online_cores as f32,
        if t.max_online_cores == 0 {
            format!("Todos ({} núcleos)", topo.physical_cores.max(1))
        } else {
            format!("{} núcleo(s)", t.max_online_cores)
        },
        ("Todos".to_owned(), max_cores_cap.to_string()),
        |v| Message::PowerMaxOnlineCores(v.round() as u32),
        None,
    ));

    row::group(TITLES_CPU[kind_index(kind)], Some("Os padrões já são otimizados."), true, rows)
}

pub(super) fn gpu_group<'a>(
    app: &'a ControlPanel,
    kind: PowerProfileKind,
    t: &'a ProfileTuning,
) -> Element<'a, Message> {
    let hw = &app.snapshot.hardware;
    let mut rows: Vec<Element<'a, Message>> = Vec::new();

    if hw.has_nvidia || !hw.is_intel {
        rows.push(row::slider(
            "Limite de potência",
            Some("Teto de consumo da placa NVIDIA. Zero usa o padrão (ou o mínimo no plano Economia)."),
            0.0..=450.0,
            5.0,
            t.nvidia_power_limit_w.min(450) as f32,
            shown_or(t.nvidia_power_limit_w, "Padrão", "W"),
            ("Padrão".to_owned(), "450 W".to_owned()),
            |v| {
                let w = ((v / 5.0).round() as u32) * 5;
                Message::PowerNvidiaPowerLimit(if w < 50 { 0 } else { w })
            },
            None,
        ));
        rows.push(row::slider(
            "Travar clock da GPU",
            Some("Teto fixo da frequência do processador gráfico. Zero deixa livre."),
            0.0..=3000.0,
            50.0,
            t.nvidia_gpu_clock_mhz.min(3000) as f32,
            shown_or(t.nvidia_gpu_clock_mhz, "Livre", "MHz"),
            ("Livre".to_owned(), "3000 MHz".to_owned()),
            |v| {
                let m = ((v / 50.0).round() as u32) * 50;
                Message::PowerNvidiaGpuClock(if m < 300 { 0 } else { m })
            },
            None,
        ));
        rows.push(row::slider(
            "Travar clock da memória",
            Some("Teto fixo da frequência da memória de vídeo. Zero deixa livre."),
            0.0..=12000.0,
            100.0,
            t.nvidia_mem_clock_mhz.min(12000) as f32,
            shown_or(t.nvidia_mem_clock_mhz, "Livre", "MHz"),
            ("Livre".to_owned(), "12000 MHz".to_owned()),
            |v| {
                let m = ((v / 100.0).round() as u32) * 100;
                Message::PowerNvidiaMemClock(if m < 400 { 0 } else { m })
            },
            None,
        ));
        rows.push(row::toggle(
            "Persistence Mode",
            Some("Mantém o driver NVIDIA carregado: acorda mais rápido, mas gasta alguns watts parado."),
            t.nvidia_persistence,
            |_| Message::PowerToggleNvidiaPersistence,
        ));
    }
    if hw.is_intel {
        rows.push(row::slider(
            "iGPU Intel Arc: frequência máxima",
            Some("Teto da frequência do gráfico integrado. Zero usa o máximo do hardware."),
            0.0..=2400.0,
            50.0,
            t.intel_igpu_max_mhz.min(2400) as f32,
            shown_or(t.intel_igpu_max_mhz, "Padrão", "MHz"),
            ("Padrão".to_owned(), "2400 MHz".to_owned()),
            |v| {
                let m = ((v / 50.0).round() as u32) * 50;
                Message::PowerIntelIgpuMaxMhz(if m < 300 { 0 } else { m })
            },
            None,
        ));
    }

    row::group(TITLES_GPU[kind_index(kind)], Some("Zero = livre."), true, rows)
}

pub(super) fn platform_group<'a>(
    kind: PowerProfileKind,
    t: &'a ProfileTuning,
) -> Element<'a, Message> {
    let aspm_labels: Vec<&str> = power_config::ASPM_OPTIONS.iter().map(|&(_, l)| l).collect();
    let alpm_labels: Vec<&str> = power_config::SATA_ALPM_OPTIONS
        .iter()
        .map(|&(_, l)| l)
        .collect();

    let rows: Vec<Element<'a, Message>> = vec![
        row::segmented(
            "PCIe ASPM",
            Some("ASPM (Active State Power Management): economia de energia dos links PCIe."),
            &aspm_labels,
            marked(power_config::ASPM_OPTIONS, &t.pcie_aspm),
            |i| Message::PowerPcieAspm(power_config::ASPM_OPTIONS[i].0.to_owned()),
        ),
        row::segmented(
            "SATA ALPM",
            Some("ALPM (Aggressive Link Power Management): economia de energia dos discos SATA."),
            &alpm_labels,
            marked(power_config::SATA_ALPM_OPTIONS, &t.sata_alpm),
            |i| Message::PowerSataAlpm(power_config::SATA_ALPM_OPTIONS[i].0.to_owned()),
        ),
        row::toggle(
            "Economia de energia do áudio",
            Some("Desliga o codec de áudio (snd_hda_intel power_save) após 1 s de silêncio."),
            t.audio_power_save,
            |_| Message::PowerToggleAudioSave,
        ),
        row::toggle(
            "Economia de energia do Wi-Fi",
            Some("Pode aumentar a latência da rede sem fio (iw power_save)."),
            t.wifi_power_save,
            |_| Message::PowerToggleWifiSave,
        ),
        row::kv(
            "Arquivo de perfis",
            power_config::PowerProfilesConfig::config_path()
                .to_string_lossy()
                .into_owned(),
        ),
    ];

    row::group(TITLES_PLATFORM[kind_index(kind)], None, true, rows)
}
