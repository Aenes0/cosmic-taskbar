use cosmic::Element;
use crate::control_panel::{ControlPanel, Message, Section};
use super::row::{self, BtnStyle, Tone};
use super::shell::action_button;

/// Caminho do script de hardening no repositório.
fn harden_script(app: &ControlPanel) -> String {
    let repo_dir = app
        .snapshot
        .repo
        .path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "$HOME/Projetos/cosmic-taskbar".to_owned());
    format!("\"{repo_dir}/extras/security/apply-hardening.sh\"")
}

/// Comando da proteção recomendada (firewall + DNS Quad9 + privacidade de rede + kernel).
fn recommended_cmd(script: &str) -> String {
    format!(
        "{script} --firewall-strict && {script} --dns quad9 && {script} --nm-privacy && {script} --kernel"
    )
}

/// Índice do provedor de DNS em uso (0 Quad9, 1 Cloudflare, 2 Mullvad); `usize::MAX` se outro.
fn dns_provider_index(server: &str) -> usize {
    if server.contains("9.9.9.9") {
        0
    } else if server.contains("1.1.1.1") || server.contains("1.0.0.1") {
        1
    } else if server.contains("194.242.2.") {
        2
    } else {
        usize::MAX
    }
}

fn on_off(active: bool, on: &'static str, off: &'static str) -> (&'static str, Tone) {
    if active { (on, Tone::Ok) } else { (off, Tone::Neutral) }
}

/// Linha "ligado/desligado" com botão de alternar que roda `RunAction` (só no clique).
fn flag_row<'a>(
    label: &'a str,
    desc: &'a str,
    active: bool,
    script: &str,
    title: &'static str,
    flag: &'static str,
) -> Element<'a, Message> {
    let next = if active { "off" } else { "on" };
    let cmd = format!("{script} {flag} {next}");
    row::toggle(label, Some(desc), active, move |_| {
        Message::RunAction(title, cmd.clone(), true)
    })
}

fn essential_group<'a>(app: &'a ControlPanel, script: &str) -> Element<'a, Message> {
    let sec = &app.snapshot.security;
    let (fw_pill, fw_tone) = on_off(
        sec.ufw_active,
        "Ativo",
        if sec.ufw_installed { "Desativado" } else { "Não instalado" },
    );
    let (dns_pill, dns_tone) = on_off(sec.dns_over_tls, "Criptografado", "Sem criptografia");
    let (hosts_pill, hosts_tone) = on_off(sec.hosts_adblock_active, "Ativo", "Desativado");
    let (nm_pill, nm_tone) = on_off(sec.nm_privacy_conf, "Ativo", "Padrão");
    let (kn_pill, kn_tone) = on_off(sec.kernel_hardening_conf, "Ativo", "Padrão");
    let dns_label = if sec.dns_server.is_empty() {
        "Padrão do roteador".to_owned()
    } else {
        sec.dns_server.clone()
    };
    let hosts_cmd = if sec.hosts_adblock_active { "--hosts-undo" } else { "--hosts-adblock" };
    let nm_cmd = if sec.nm_privacy_conf { "--nm-privacy-undo" } else { "--nm-privacy" };
    let kernel_cmd = if sec.kernel_hardening_conf { "--kernel-undo" } else { "--kernel" };
    let dns_script = script.to_owned();

    row::group(
        "Proteção essencial",
        Some("Opções seguras para quase todo mundo. Cada item diz o efeito real antes do clique."),
        false,
        vec![
            row::status(
                "Firewall",
                Some("Recomendado · Impede que outros dispositivos da rede acessem seu PC. Não atrapalha navegação nem jogos."),
                fw_pill,
                fw_tone,
                vec![row::button(
                    if sec.ufw_active { "Reaplicar" } else { "Ativar" },
                    Message::RunAction("Firewall UFW", format!("{script} --firewall-strict"), true),
                    BtnStyle::Normal,
                )],
            ),
            row::status(
                "DNS criptografado",
                Some("Recomendado · Esconde do provedor e de redes Wi-Fi os sites que você visita. Escolha o provedor abaixo."),
                dns_pill,
                dns_tone,
                Vec::new(),
            ),
            row::segmented(
                "Provedor de DNS",
                Some("Quad9 bloqueia golpes · Cloudflare é mais rápido · Mullvad não guarda registros."),
                &["Quad9", "Cloudflare", "Mullvad"],
                dns_provider_index(&sec.dns_server),
                move |i| {
                    let (title, name) = match i {
                        0 => ("DNS Quad9", "quad9"),
                        1 => ("DNS Cloudflare", "cloudflare"),
                        _ => ("DNS Mullvad", "mullvad"),
                    };
                    Message::RunAction(title, format!("{dns_script} --dns {name}"), true)
                },
            ),
            row::kv("DNS em uso", dns_label),
            row::status(
                "Bloquear rastreadores",
                Some("Opcional · Bloqueia anúncios e rastreadores em todos os apps. Pode bloquear alguns links patrocinados de busca."),
                hosts_pill,
                hosts_tone,
                vec![row::button(
                    if sec.hosts_adblock_active { "Desativar" } else { "Ativar" },
                    Message::RunAction("Bloqueio de Rastreadores", format!("{script} {hosts_cmd}"), true),
                    BtnStyle::Normal,
                )],
            ),
            row::status(
                "Privacidade de Wi-Fi e rede",
                Some("Recomendado · Esconde o identificador real ao procurar redes Wi-Fi e desativa testes automáticos de conexão."),
                nm_pill,
                nm_tone,
                vec![row::button(
                    if sec.nm_privacy_conf { "Desativar" } else { "Ativar" },
                    Message::RunAction("Privacidade de Rede", format!("{script} {nm_cmd}"), true),
                    BtnStyle::Normal,
                )],
            ),
            row::status(
                "Proteção básica do kernel",
                Some("Recomendado · Impede que programas comuns espionem a memória de outros apps. Seguro para uso diário."),
                kn_pill,
                kn_tone,
                vec![row::button(
                    if sec.kernel_hardening_conf { "Desativar" } else { "Ativar" },
                    Message::RunAction("Proteção do Sistema", format!("{script} {kernel_cmd}"), true),
                    BtnStyle::Normal,
                )],
            ),
        ],
    )
}

fn privacy_group(app: &ControlPanel) -> Element<'_, Message> {
    let sec = &app.snapshot.security;
    let mic_muted = app.microphone.unwrap_or(false);
    let cam_in_use = !app.privacy.camera.is_empty();
    let cam_meta = if cam_in_use {
        format!("Em uso por: {}", app.privacy.camera.join(", "))
    } else {
        "Nenhum app usando a câmera agora".to_owned()
    };
    let mic_meta = if app.privacy.microphone.is_empty() {
        "Nenhum app gravando microfone agora".to_owned()
    } else {
        format!("Em uso por: {}", app.privacy.microphone.join(", "))
    };
    let (kill_label, kill_cmd) = if sec.networking_enabled {
        ("Cortar internet", "nmcli networking off && echo 'Internet desligada.' ")
    } else {
        ("Religar internet", "nmcli networking on && echo 'Internet religada.' ")
    };
    let clean_cmd = concat!(
        "rm -rf \"${XDG_DATA_HOME:-$HOME/.local/share}/cosmic-central/clipboard\"* ",
        "\"${XDG_DATA_HOME:-$HOME/.local/share}/recently-used.xbel\" ",
        "\"${XDG_CACHE_HOME:-$HOME/.cache}/thumbnails\"/* 2>/dev/null || true; ",
        "systemctl --user restart central-clipboard.service 2>/dev/null || true; ",
        "echo 'Rastros limpos (Área de transferência, arquivos recentes e miniaturas).'"
    );
    let vpn = !sec.vpn_iface.is_empty();

    row::group(
        "Privacidade agora",
        None,
        false,
        vec![
            row::list_item(
                "Câmera",
                cam_meta,
                Some(if cam_in_use { ("Em uso", Tone::Warn) } else { ("Livre", Tone::Ok) }),
                Some(row::button(
                    "Câmeras & Áudio",
                    Message::SelectSection(Section::Camera),
                    BtnStyle::Normal,
                )),
            ),
            row::list_item(
                "Microfone",
                mic_meta,
                Some(if mic_muted { ("Silenciado", Tone::Ok) } else { ("Ativo", Tone::Neutral) }),
                Some(row::button(
                    if mic_muted { "Reativar" } else { "Silenciar" },
                    Message::ToggleMicrophone,
                    BtnStyle::Normal,
                )),
            ),
            row::list_item(
                "VPN",
                "Mostra se há um túnel conectado (WireGuard, OpenVPN ou Tailscale).".to_owned(),
                Some(if vpn { ("Conectada", Tone::Ok) } else { ("Sem VPN", Tone::Neutral) }),
                Some(row::button(
                    "Rede & VPN",
                    Message::SelectSection(Section::Network),
                    BtnStyle::Normal,
                )),
            ),
            row::note("A troca automática do endereço MAC agora fica em Rede & VPN."),
            row::buttons(
                "Kill switch da internet",
                Some(if sec.networking_enabled {
                    "Corta toda a rede na hora. Clique de novo para reconectar."
                } else {
                    "A internet está desligada. Clique para reconectar."
                }),
                vec![row::button(
                    kill_label,
                    Message::RunAction("Internet (Kill Switch)", kill_cmd.to_owned(), false),
                    BtnStyle::Danger,
                )],
            ),
            row::buttons(
                "Limpar rastros",
                Some("Área de transferência, arquivos recentes e miniaturas."),
                vec![row::button(
                    "Limpar agora",
                    Message::RunAction("Limpeza de Rastros", clean_cmd.to_owned(), false),
                    BtnStyle::Normal,
                )],
            ),
            row::buttons(
                "Bloquear tela",
                None,
                vec![row::button("Bloquear", Message::GreeterTestLock, BtnStyle::Normal)],
            ),
        ],
    )
}

fn shield_group<'a>(app: &'a ControlPanel, script: &str) -> Element<'a, Message> {
    let sec = &app.snapshot.security;
    let (usb_pill, usb_tone) = on_off(
        sec.usbguard_active,
        "Ativo",
        if sec.usbguard_installed { "Desativado" } else { "Não instalado" },
    );
    let usb_cmd = if sec.usbguard_active { "--usbguard-disable" } else { "--usbguard-setup" };
    row::group(
        "Blindagem avançada",
        Some("Ajustes do kernel e da rede. Avançado: leia o efeito antes de ligar."),
        true,
        vec![
            flag_row(
                "Desativar coredumps",
                "Recomendado · Evita salvar a RAM em disco quando um app trava (protege senhas).",
                sec.suid_dumpable == 0 && sec.nocoredump_conf,
                script,
                "Bloquear Coredumps",
                "--toggle-nocoredump",
            ),
            flag_row(
                "Bloquear módulos legados",
                "Avançado · Desliga drivers antigos (FireWire, HFS, DCCP/SCTP) que quase ninguém usa.",
                sec.blacklist_modules_conf,
                script,
                "Blacklist de Módulos",
                "--toggle-blacklist",
            ),
            flag_row(
                "Desativar io_uring",
                "Cuidado · Fecha uma superfície frequente de falhas no kernel. Pode quebrar bancos de dados e benchmarks.",
                sec.io_uring_disabled >= 1,
                script,
                "Restringir io_uring",
                "--toggle-io-uring",
            ),
            flag_row(
                "ICMP furtivo",
                "Avançado · Seu PC para de responder a ping. Pode dificultar testes de rede local.",
                sec.icmp_stealth,
                script,
                "Stealth ICMP",
                "--toggle-icmp-stealth",
            ),
            flag_row(
                "Desativar Magic SysRq",
                "Avançado · Desliga os atalhos de emergência do kernel. Deixe desligado se usa REISUB para destravar o PC.",
                sec.sysrq_disabled,
                script,
                "Magic SysRq",
                "--toggle-sysrq",
            ),
            row::status(
                "USBGuard",
                Some("Cuidado · Só libera os dispositivos USB já conectados hoje; novos pendrives e teclados precisarão de autorização."),
                usb_pill,
                usb_tone,
                vec![row::button(
                    if sec.usbguard_active { "Desativar" } else { "Ativar" },
                    Message::RunAction("USBGuard", format!("{script} {usb_cmd}"), true),
                    BtnStyle::Normal,
                )],
            ),
            row::buttons(
                "Blindagem completa",
                Some("Aplica kernel, armadura do kernel e privacidade de rede de uma vez."),
                vec![
                    row::button(
                        "Reverter",
                        Message::RunAction(
                            "Reverter Blindagem",
                            format!("{script} --kernel-armor-undo"),
                            true,
                        ),
                        BtnStyle::Normal,
                    ),
                    row::button(
                        "Aplicar tudo",
                        Message::RunAction(
                            "Blindagem Completa",
                            format!("{script} --kernel && {script} --kernel-armor && {script} --nm-privacy"),
                            true,
                        ),
                        BtnStyle::Primary,
                    ),
                ],
            ),
        ],
    )
}

fn audit_group(app: &ControlPanel) -> Element<'_, Message> {
    let sec = &app.snapshot.security;
    let opt = &app.snapshot.optional;
    let exposed = if sec.exposed_ports_count == 0 {
        "Nenhuma porta exposta".to_owned()
    } else {
        format!("{} porta(s) aberta(s) para a rede", sec.exposed_ports_count)
    };
    let lockdown_ok = sec.lockdown_mode == "integrity" || sec.lockdown_mode == "confidentiality";
    let kernel = if sec.kernel_release.is_empty() { "—" } else { sec.kernel_release.as_str() };
    let lockdown = if lockdown_ok { sec.lockdown_mode.as_str() } else { "sem lockdown" };
    let service = |name: &'static str, meta: &'static str, active: bool, on_tone: Tone| {
        row::list_item(
            name,
            meta.to_owned(),
            Some(if active { ("Ativo", on_tone) } else { ("Inativo", Tone::Ok) }),
            None,
        )
    };
    let term = |label: &'static str, title: &'static str, cmd: String| {
        row::button(label, Message::OpenInTerminal(title, cmd), BtnStyle::Normal)
    };
    let flatpak_cmd = if sec.flatpak_installed {
        "for a in $(flatpak list --app --columns=application); do perms=$(flatpak info --show-permissions \"$a\" 2>/dev/null | grep -E 'filesystems=.*host|sockets=.*x11|devices=all'); [ -n \"$perms\" ] && printf '\\n[%s]\\n%s\\n' \"$a\" \"$perms\"; done"
    } else {
        "echo 'Flatpak não instalado.'"
    };

    row::group(
        "Auditoria",
        None,
        true,
        vec![
            row::kv("Portas de rede abertas", format!("{exposed} · {} locais", sec.local_ports_count)),
            row::kv("Criptografia de disco (LUKS)", if sec.luks_active { "Ativa" } else { "Não detectada" }.to_owned()),
            row::kv(
                "Tela de login",
                if opt.greeter_service_enabled { "Boot + sessão isolados" } else { "Sessão isolada" }.to_owned(),
            ),
            row::kv("Kernel / Lockdown", format!("{kernel} · {lockdown}")),
            service("SSH", "sshd.service", sec.sshd_active, Tone::Warn),
            service("Avahi", "Descoberta na rede local", sec.avahi_active, Tone::Warn),
            service("Impressão", "cups.service", sec.cups_active, Tone::Info),
            service("Bluetooth", "bluetooth.service", sec.bluetooth_active, Tone::Info),
            row::buttons(
                "Ferramentas",
                Some("Abrem no terminal."),
                vec![
                    term("Regras do firewall", "Regras UFW", "sudo ufw status verbose".to_owned()),
                    term("Conexões ativas", "Conexões Ativas", "sudo ss -tunp state established".to_owned()),
                    term("Portas abertas", "Portas Abertas", "sudo ss -tulnp".to_owned()),
                    term(
                        "Verificar pacotes",
                        "Integridade do Sistema",
                        "pacman -Qkk 2>&1 | grep -Ev '0 arquivos alterados|0 altered files' | head -n 60; echo '--- Fim da auditoria ---'".to_owned(),
                    ),
                    term("Permissões Flatpak", "Permissões Flatpak", flatpak_cmd.to_owned()),
                ],
            ),
        ],
    )
}

pub(super) fn security_view(app: &ControlPanel) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let script = harden_script(app);
    let mut items: Vec<Element<'_, Message>> = vec![
        essential_group(app, &script),
        privacy_group(app),
    ];
    if app.advanced_open {
        items.push(shield_group(app, &script));
        items.push(audit_group(app));
    }
    cosmic::widget::column::with_children(items)
        .spacing(spacing.space_m)
        .into()
}

/// Botões do cabeçalho da página.
pub(super) fn security_actions(app: &ControlPanel) -> Vec<Element<'_, Message>> {
    let script = harden_script(app);
    let done = {
        let s = &app.snapshot.security;
        s.ufw_active && s.dns_over_tls && s.nm_privacy_conf && s.kernel_hardening_conf
    };
    vec![
        action_button("Verificar agora", Message::RefreshAll, BtnStyle::Normal, !app.busy),
        action_button(
            if done { "Reaplicar recomendado" } else { "Ativar proteção recomendada" },
            Message::RunAction("Proteção Recomendada", recommended_cmd(&script), true),
            BtnStyle::Primary,
            !app.busy,
        ),
    ]
}

/// Selo ao lado do título: "Bem protegido · 92/100" (verde), "Proteção boa · N/100" ou
/// "Pode melhorar · N/100" (atenção).
pub(super) fn security_badge(app: &ControlPanel) -> Option<(String, Tone)> {
    let score = app.snapshot.security.score;
    let (label, tone) = if score >= 90 {
        ("Bem protegido", Tone::Ok)
    } else if score >= 80 {
        ("Proteção boa", Tone::Ok)
    } else {
        ("Pode melhorar", Tone::Warn)
    };
    Some((format!("{label} · {score}/100"), tone))
}
