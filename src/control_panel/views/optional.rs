use cosmic::Element;
use crate::control_panel::{ControlPanel, Message, Section};
use super::row::{self, BtnStyle, Tone};

fn install_cmd(helper: &str, pkg: &str) -> String {
    format!(
        "if command -v paru >/dev/null 2>&1; then paru -S --needed {pkg}; elif command -v yay >/dev/null 2>&1; then yay -S --needed {pkg}; else echo 'Instale {helper} via AUR (paru/yay)'; fi"
    )
}

fn install_btn<'a>(title: &'static str, cmd: String) -> Element<'a, Message> {
    row::button("Instalar", Message::OpenInTerminal(title, cmd), BtnStyle::Primary)
}

fn state(installed: bool, active: bool) -> (&'static str, Tone) {
    if active {
        ("Ativo", Tone::Ok)
    } else if installed {
        ("Instalado", Tone::Ok)
    } else {
        ("Não instalado", Tone::Neutral)
    }
}

pub(super) fn optional_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let opt = &app.snapshot.optional;
    let repo_dir = app
        .snapshot
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());

    let switcher_btn = if opt.switcher_active {
        row::button(
            "Reiniciar",
            Message::RestartUserUnit("cosmic-switcher.service"),
            BtnStyle::Normal,
        )
    } else if opt.switcher_installed {
        row::button(
            "Habilitar",
            Message::EnableUserUnit("cosmic-switcher.service"),
            BtnStyle::Primary,
        )
    } else {
        install_btn(
            "Instalar aenes-switcher",
            format!("cd \"{repo_dir}\" && ./install.sh && ./setup.sh"),
        )
    };
    let iriun_btn = if opt.iriun_installed {
        row::button("Gerenciar", Message::SelectSection(Section::Camera), BtnStyle::Normal)
    } else {
        install_btn(
            "Instalar IriunCam",
            format!("cd \"{repo_dir}\" && ./scripts/bootstrap-pc.sh --with-iriun"),
        )
    };
    let cooler_btn = if opt.coolercontrol_installed {
        row::button(
            "Abrir",
            Message::SpawnApp("coolercontrol", vec![], "CoolerControl"),
            BtnStyle::Normal,
        )
    } else {
        install_btn("Instalar CoolerControl", install_cmd("coolercontrol", "coolercontrol"))
    };
    let mc_btn = if !opt.mission_center_installed {
        install_btn(
            "Instalar Mission Center",
            "sudo pacman -S --needed mission-center || flatpak install -y flathub io.missioncenter.MissionCenter".to_owned(),
        )
    } else if opt.mission_center_flatpak {
        row::button(
            "Abrir",
            Message::SpawnApp("flatpak", vec!["run", "io.missioncenter.MissionCenter"], "Mission Center"),
            BtnStyle::Normal,
        )
    } else {
        row::button(
            "Abrir",
            Message::SpawnApp("missioncenter", vec![], "Mission Center"),
            BtnStyle::Normal,
        )
    };
    let brave_btn = if opt.brave_installed {
        row::button("Abrir", Message::SpawnApp("brave", vec![], "Brave"), BtnStyle::Normal)
    } else {
        install_btn(
            "Instalar Brave",
            "if command -v paru >/dev/null 2>&1; then paru -S --needed brave-bin; elif command -v yay >/dev/null 2>&1; then yay -S --needed brave-bin; else sudo pacman -S --needed brave-bin; fi".to_owned(),
        )
    };

    let item = |name: &'static str, meta: &str, st: (&'static str, Tone), btn| {
        row::list_item(name, meta.to_owned(), Some(st), Some(btn))
    };
    let rows = vec![
        item(
            "aenes-switcher",
            "Alt+Tab / Super+Tab isolado",
            state(opt.switcher_installed, opt.switcher_active),
            switcher_btn,
        ),
        item(
            "IriunCam",
            "Celular como webcam",
            state(opt.iriun_installed, false),
            iriun_btn,
        ),
        item(
            "CoolerControl",
            "Ventoinhas e curvas térmicas",
            state(opt.coolercontrol_installed, opt.coolercontrol_active),
            cooler_btn,
        ),
        item(
            "Mission Center",
            "Gerenciador de tarefas",
            state(opt.mission_center_installed, false),
            mc_btn,
        ),
        item("Brave", "Navegador", state(opt.brave_installed, false), brave_btn),
    ];
    cosmic::widget::column::with_children(vec![row::group("Pacotes", Some("Instalar abre um terminal com o comando, para você ver o que roda."), false, rows)])
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página.
pub(super) fn optional_actions(_app: &ControlPanel) -> Vec<Element<'_, Message>> {
    Vec::new()
}

/// Selo ao lado do título: sem selo por enquanto.
pub(super) fn optional_badge(_app: &ControlPanel) -> Option<(String, Tone)> {
    None
}
