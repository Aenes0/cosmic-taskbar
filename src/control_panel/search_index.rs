// SPDX-License-Identifier: GPL-3.0-only

//! Índice de busca da paleta Ctrl K (entrega painel-b).
//!
//! Os rótulos vêm do mockup (artboard `Main`, função `data()`); as páginas nativas são
//! argumentos aceitos por `cosmic-settings --help`.

use super::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Setting,
    Action,
    Native,
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    pub section: Section,
    pub group: &'static str,
    pub label: &'static str,
    pub keywords: &'static str,
    pub advanced: bool,
    /// Argumento de `cosmic-settings` (só nas entradas `Native`).
    pub native_page: Option<&'static str>,
}

const MAX_SETTINGS: usize = 5;
const MAX_ACTIONS: usize = 3;
const MAX_NATIVE: usize = 3;
const ACTIONS_GROUP: &str = "Ações da página";
const NATIVE_GROUP: &str = "Configurações do COSMIC";

/// Ajuste comum.
macro_rules! st {
    ($sec:ident, $g:expr, $l:expr, $k:expr) => {
        Entry { kind: EntryKind::Setting, section: Section::$sec, group: $g, label: $l, keywords: $k, advanced: false, native_page: None }
    };
}
/// Ajuste de grupo avançado.
macro_rules! sa {
    ($sec:ident, $g:expr, $l:expr, $k:expr) => {
        Entry { kind: EntryKind::Setting, section: Section::$sec, group: $g, label: $l, keywords: $k, advanced: true, native_page: None }
    };
}
/// Botão de ação do cabeçalho da página.
macro_rules! ac {
    ($sec:ident, $l:expr, $k:expr) => {
        Entry { kind: EntryKind::Action, section: Section::$sec, group: ACTIONS_GROUP, label: $l, keywords: $k, advanced: false, native_page: None }
    };
}
/// Página nativa do `cosmic-settings`.
macro_rules! nt {
    ($sec:ident, $page:expr, $l:expr, $k:expr) => {
        Entry { kind: EntryKind::Native, section: Section::$sec, group: NATIVE_GROUP, label: $l, keywords: $k, advanced: false, native_page: Some($page) }
    };
}

pub static INDEX: &[Entry] = &[
    // ── Energia & Bateria ──
    st!(Power, "Plano ativo", "power-profiles-daemon", "plano economia equilibrado desempenho servico"),
    st!(Power, "Tela e bloqueio", "Desligar a tela", "monitor apagar ocioso"),
    st!(Power, "Tela e bloqueio", "Pedir senha (Tela de Login)", "bloqueio bloquear senha idle"),
    st!(Power, "Tela e bloqueio", "Suspender na tomada", "dormir ac"),
    st!(Power, "Tela e bloqueio", "Suspender na bateria", "dormir notebook"),
    st!(Power, "Tela e bloqueio", "Taxa de atualização (DP-3)", "hz refresh monitor"),
    st!(Power, "Bateria", "Modo conservação Lenovo", "bateria limite carga 80 preservar"),
    st!(Power, "Bateria", "Mostrar porcentagem da bateria na barra", "bateria percentual barra"),
    st!(Power, "Periféricos", "Mouse sem fio", "bateria bolt logitech"),
    st!(Power, "Periféricos", "Teclado sem fio", "bateria bolt logitech"),
    sa!(Power, "CPU", "Boost / Turbo", "frequencia cpu"),
    sa!(Power, "CPU", "Teto de frequência", "mhz cpu clock"),
    sa!(Power, "CPU", "Preferência de eficiência (EPP)", "epp balance_power cpu"),
    sa!(Power, "CPU", "Limite sustentado (RAPL PL1)", "watts potencia pl1 cpu"),
    sa!(Power, "CPU", "Pico curto (RAPL PL2)", "watts potencia pl2 cpu"),
    sa!(Power, "CPU", "Desligar CCD1", "ccd nucleos ryzen"),
    sa!(Power, "CPU", "Núcleos online", "cores threads cpu"),
    sa!(Power, "GPU", "Limite de potência", "watts nvidia gpu"),
    sa!(Power, "GPU", "Travar clock da GPU", "mhz nvidia"),
    sa!(Power, "GPU", "Travar clock da memória", "mhz nvidia vram"),
    sa!(Power, "GPU", "Persistence Mode", "nvidia driver"),
    sa!(Power, "Plataforma", "PCIe ASPM", "link pcie"),
    sa!(Power, "Plataforma", "SATA ALPM", "disco sata"),
    sa!(Power, "Plataforma", "Economia de energia do áudio", "power_save codec"),
    sa!(Power, "Plataforma", "Economia de energia do Wi-Fi", "wifi latencia"),
    sa!(Power, "Kernel, módulos e regras", "Daemon economia-energia", "servico"),
    sa!(Power, "Kernel, módulos e regras", "Áudio power_save no boot", "audio"),
    sa!(Power, "Kernel, módulos e regras", "Leitura RAPL em watts", "rapl"),
    sa!(Power, "Kernel, módulos e regras", "Módulo i2c-dev", "kernel modulo"),
    sa!(Power, "Kernel, módulos e regras", "Módulo nct6775", "kernel modulo sensores placa-mae"),
    sa!(Power, "Kernel, módulos e regras", "Instalar ou atualizar regras", "udev administrador"),
    ac!(Power, "Restaurar padrões", "resetar energia"),
    // ── Monitores & Janelas ──
    st!(Displays, "DP-3 · LG ULTRAWIDE", "Monitor principal", "primario tela barra"),
    st!(Displays, "DP-3 · LG ULTRAWIDE", "Brilho (DDC)", "luminosidade tela"),
    st!(Displays, "DP-1 · Samsung S22B300", "Tornar principal", "primario monitor"),
    st!(Displays, "DP-1 · Samsung S22B300", "Brilho (DDC)", "luminosidade tela"),
    st!(Displays, "Janelas", "Espaçamento", "gap padding janelas"),
    st!(Displays, "Janelas", "Margem externa da tela", "padding borda"),
    st!(Displays, "Janelas", "Espaço entre janelas divididas", "gap"),
    st!(Displays, "Janelas", "Prioridade de foco", "roubar foco atencao"),
    st!(Displays, "Utilitários visuais", "Wallpapers por monitor", "papel de parede slideshow recorte"),
    st!(Displays, "Utilitários visuais", "Ícones PWA do Brave", "brave icones pwa"),
    ac!(Displays, "Ler monitores", "detectar telas"),
    // ── Rede & VPN ──
    st!(Network, "Conexão ativa", "Conexão", "ethernet cabeada"),
    st!(Network, "Conexão ativa", "Gateway", "ip rota"),
    st!(Network, "Conexão ativa", "Endereço MAC", "mac"),
    st!(Network, "Wi-Fi", "Wi-Fi", "wifi sem fio radio"),
    st!(Network, "VPNs", "wg-casa", "wireguard vpn"),
    st!(Network, "Privacidade", "Trocar endereço MAC periodicamente", "mac-rotativo timer"),
    ac!(Network, "Abrir no COSMIC", "networkmanager rede"),
    // ── Câmeras & Áudio ──
    st!(Camera, "Privacidade", "Chave física da câmera", "webcam"),
    st!(Camera, "Privacidade", "Microfone interno", "mic mudo"),
    st!(Camera, "Câmeras detectadas", "IriunCam", "webcam virtual video0"),
    st!(Camera, "IriunCam · celular como webcam", "Estado", "iriun webcam celular"),
    ac!(Camera, "Reescanear", "camera detectar"),
    // ── Barra & Tray ──
    st!(Bar, "Ícones na barra", "Espaçamento entre ícones", "tray"),
    st!(Bar, "Ícones na barra", "Rede", "tray icone"),
    st!(Bar, "Ícones na barra", "Volume", "tray icone som"),
    st!(Bar, "Ícones na barra", "Câmera e mic.", "tray icone"),
    st!(Bar, "Ícones na barra", "Câmera", "tray icone"),
    st!(Bar, "Ícones na barra", "Energia", "tray icone bateria"),
    st!(Bar, "Ícones na barra", "Relógio", "tray icone hora"),
    st!(Bar, "Ícones na barra", "Notificações", "tray icone"),
    st!(Bar, "Ícones na barra", "Privacidade", "tray icone"),
    st!(Bar, "Ícones na barra", "Bluetooth", "tray icone"),
    st!(Bar, "Ícones na barra", "Não perturbe", "tray icone"),
    st!(Bar, "Ícones na barra", "Modo escuro", "tray icone tema"),
    st!(Bar, "Ícones na barra", "Periféricos", "tray icone"),
    st!(Bar, "Apps em segundo plano ocultos", "Apps ocultos", "background tray"),
    ac!(Bar, "Reiniciar painel", "recarregar barra panel"),
    // ── Central (Direita) ──
    st!(Central, "Aparência", "Modo escuro", "tema escuro dark"),
    st!(Central, "Aparência", "Não perturbe", "silenciar notificacoes dnd"),
    st!(Central, "Seções abertas por padrão", "Notificações", "secao"),
    st!(Central, "Seções abertas por padrão", "Reprodutor de mídia", "musica player secao"),
    st!(Central, "Seções abertas por padrão", "Calendário", "secao"),
    st!(Central, "Seções abertas por padrão", "Área de transferência", "clipboard secao"),
    st!(Central, "Janelas lado a lado", "Ladrilhar novos espaços de trabalho", "tiling workspaces"),
    st!(Central, "Janelas lado a lado", "Destacar a janela ativa", "borda hint foco"),
    st!(Central, "Entrada e extensões", "Touchpad", "mouse desligar"),
    st!(Central, "Entrada e extensões", "Hot Edge", "cosmic-ext-hot-edge borda"),
    ac!(Central, "Restaurar tiles", "tiles rapidos layout"),
    // ── Monitor (Esquerda) ──
    st!(Monitor, "Métricas na barra", "CPU", "metrica quando alto"),
    st!(Monitor, "Métricas na barra", "GPU", "metrica quando alto"),
    st!(Monitor, "Métricas na barra", "Memória", "ram metrica"),
    st!(Monitor, "Métricas na barra", "Rede", "metrica"),
    st!(Monitor, "Métricas na barra", "Termômetro", "temperatura"),
    st!(Monitor, "Métricas na barra", "Intervalo de leitura", "atualizacao"),
    st!(Monitor, "Cartões do painel", "Discos", "cartao"),
    st!(Monitor, "Cartões do painel", "Clima", "cartao tempo"),
    st!(Monitor, "Processos", "Quantidade exibida", "processos"),
    st!(Monitor, "Processos", "Ordenar por", "processos"),
    st!(Monitor, "Clima", "Local", "cidade clima"),
    st!(Monitor, "Clima", "Buscar local", "cep cidade clima"),
    st!(Monitor, "Clima", "Cartão de clima", "tempo"),
    st!(Monitor, "Consumo elétrico", "Tarifa (R$ por kWh)", "custo energia"),
    st!(Monitor, "Consumo elétrico", "Resto do PC (W)", "watts consumo"),
    ac!(Monitor, "Restaurar padrão", "resetar monitor"),
    // ── Notificações ──
    st!(Notifications, "Prévia", "Prévia", "preview cartao"),
    st!(Notifications, "Comportamento", "Tempo na tela", "duracao toast"),
    st!(Notifications, "Comportamento", "Formato", "completo compacto minimo"),
    st!(Notifications, "Comportamento", "Posição", "topo direita canto"),
    st!(Notifications, "Aparência", "Opacidade", "transparencia"),
    st!(Notifications, "Aparência", "Raio da borda", "arredondado"),
    st!(Notifications, "Aparência", "Linhas do corpo", "texto"),
    st!(Notifications, "Aparência", "Mostrar corpo da mensagem", "texto"),
    st!(Notifications, "Aparência", "Mostrar botões de ação", "acoes"),
    st!(Notifications, "Aparência", "Agrupar por aplicativo", "grupo"),
    st!(Notifications, "Cores", "Destaque", "cor accent icone botoes"),
    st!(Notifications, "Cores", "Fundo dos cartões", "cor grafite"),
    st!(Notifications, "Cores", "Cor personalizada", "hex rrggbb"),
    ac!(Notifications, "Restaurar padrão", "resetar notificacoes"),
    ac!(Notifications, "Enviar teste", "testar notificacao"),
    // ── OSD de Dispositivos ──
    st!(DevicesOsd, "Serviço", "logi-osd", "osd logitech servico"),
    st!(DevicesOsd, "Duração e alertas", "Duração da pílula", "osd ms"),
    st!(DevicesOsd, "Duração e alertas", "Alerta de bateria baixa", "osd porcentagem"),
    st!(DevicesOsd, "Duração e alertas", "Alerta de bateria crítica", "osd porcentagem"),
    st!(DevicesOsd, "Duração e alertas", "Verificar bateria a cada", "osd intervalo"),
    st!(DevicesOsd, "Tecla de emoji", "Comando da tecla Emoji", "smile cosmic-launcher"),
    ac!(DevicesOsd, "Testar OSD", "osd teste pilula"),
    // ── Tela de Login ──
    st!(Login, "Fora deste redesenho", "Tela de Login", "login bloqueio greeter senha avatar fundo"),
    // ── Captura de tela ──
    st!(Capture, "Depois de capturar", "Ação", "print screenshot captura recorte satty"),
    st!(Capture, "Depois de capturar", "Copiar para a área de transferência", "print screenshot clipboard recorte"),
    st!(Capture, "Depois de capturar", "Tocar som", "print screenshot audio recorte"),
    st!(Capture, "Depois de capturar", "Tempo das ações da notificação", "print screenshot recorte"),
    st!(Capture, "Depois de capturar", "Pasta de destino", "print screenshot imagens recorte"),
    st!(Capture, "Seleção", "Cor da borda", "print screenshot recorte selecao"),
    st!(Capture, "Seleção", "Espessura da borda", "print screenshot recorte selecao"),
    st!(Capture, "Seleção", "Escurecimento do fundo", "print screenshot recorte selecao"),
    st!(Capture, "Editor Satty", "Ferramenta inicial", "print screenshot satty seta recorte"),
    ac!(Capture, "Abrir pasta", "print screenshot imagens"),
    ac!(Capture, "Testar recorte", "print screenshot captura super shift s"),
    // ── Segurança & Privacidade ──
    st!(Security, "Proteção essencial", "Firewall", "ufw rede"),
    st!(Security, "Proteção essencial", "DNS criptografado", "dot dns"),
    st!(Security, "Proteção essencial", "Provedor de DNS", "quad9 cloudflare mullvad"),
    st!(Security, "Proteção essencial", "Bloquear rastreadores", "trackers anuncios"),
    st!(Security, "Proteção essencial", "Privacidade de Wi-Fi e rede", "mac wifi"),
    st!(Security, "Proteção essencial", "Proteção básica do kernel", "sysctl"),
    st!(Security, "Privacidade agora", "Câmera", "webcam privacidade"),
    st!(Security, "Privacidade agora", "Microfone", "mic privacidade"),
    st!(Security, "Privacidade agora", "Kill switch da internet", "cortar internet rede"),
    st!(Security, "Privacidade agora", "Limpar rastros", "recentes miniaturas clipboard"),
    st!(Security, "Privacidade agora", "Bloquear tela", "lock bloqueio"),
    sa!(Security, "Blindagem avançada", "Desativar coredumps", "kernel"),
    sa!(Security, "Blindagem avançada", "Bloquear módulos legados", "kernel"),
    sa!(Security, "Blindagem avançada", "Desativar io_uring", "kernel"),
    sa!(Security, "Blindagem avançada", "ICMP furtivo", "ping"),
    sa!(Security, "Blindagem avançada", "Desativar Magic SysRq", "sysrq reisub kernel"),
    sa!(Security, "Blindagem avançada", "USBGuard", "usb dispositivos"),
    sa!(Security, "Blindagem avançada", "Blindagem completa", "aplicar reverter hardening"),
    sa!(Security, "Auditoria", "SSH", "sshd"),
    sa!(Security, "Auditoria", "Avahi", "descoberta rede local"),
    sa!(Security, "Auditoria", "Impressão", "cups impressora"),
    sa!(Security, "Auditoria", "Bluetooth", "bluetooth.service"),
    sa!(Security, "Auditoria", "Ferramentas", "regras firewall conexoes ativas terminal"),
    ac!(Security, "Verificar agora", "auditar seguranca"),
    ac!(Security, "Reaplicar recomendado", "hardening firewall dns"),
    // ── Backup & Sincronização ──
    st!(Backup, "Backup diário · Restic", "Agendamento", "restic timer"),
    st!(Backup, "Backup diário · Restic", "Destino", "devdrive montado"),
    st!(Backup, "Backup diário · Restic", "Retenção", "diarios semanais"),
    st!(Backup, "Backup diário · Restic", "Snapshots", "listar restic"),
    st!(Backup, "Btrfs e SSD", "Snapper", "snapshots btrfs timeline"),
    st!(Backup, "Btrfs e SSD", "fstrim.timer", "ssd trim"),
    st!(Backup, "Sincronização do Claude", "Sync local", "contas claude"),
    st!(Backup, "Sincronização do Claude", "Outro PC", "exportar pacote claude"),
    ac!(Backup, "Executar backup", "restic agora"),
    // ── Pacotes & Opcionais ──
    st!(Optional, "Pacotes", "aenes-switcher", "alt tab super tab"),
    st!(Optional, "Pacotes", "IriunCam", "webcam celular"),
    st!(Optional, "Pacotes", "CoolerControl", "ventoinhas curvas termicas fan"),
    st!(Optional, "Pacotes", "Mission Center", "gerenciador de tarefas"),
    st!(Optional, "Pacotes", "Brave", "navegador"),
    // ── Atualizações & Serviços ──
    st!(System, "Atualizador automático", "Atualizar a cada boot", "update timer"),
    st!(System, "Atualizador automático", "Repositório", "github"),
    st!(System, "Atualizador automático", "Perfil visual", "exportar preferencias"),
    st!(System, "Serviços em segundo plano", "central-panels", "servico paineis"),
    st!(System, "Serviços em segundo plano", "central-clipboard", "servico area de transferencia"),
    st!(System, "Serviços em segundo plano", "central-notifications", "servico daemon"),
    st!(System, "Serviços em segundo plano", "cosmic-switcher", "servico alt tab"),
    st!(System, "Serviços em segundo plano", "logi-osd", "servico osd"),
    st!(System, "Serviços em segundo plano", "power-profiles-daemon", "servico planos de energia"),
    ac!(System, "Verificar", "atualizacoes"),
    ac!(System, "Atualizar agora", "update"),
    // ── Páginas nativas do cosmic-settings ──
    nt!(Power, "power", "Tela e bloqueio", "energia suspender senha desligar tela"),
    nt!(Displays, "displays", "Arranjo dos monitores", "monitores resolucao escala posicao tela"),
    nt!(Bar, "keyboard", "Atalhos de teclado", "atalhos teclas super"),
    nt!(Network, "bluetooth", "Bluetooth", "pareamento fones"),
    nt!(Displays, "wallpaper", "Papel de parede", "wallpaper fundo imagem tela"),
    nt!(Camera, "sound", "Som", "audio volume saida entrada microfone"),
    nt!(System, "time", "Data e hora", "relogio fuso horario calendario"),
    nt!(Login, "users", "Usuários", "usuario conta senha avatar"),
];

/// Minúsculas e sem acento (á→a, ç→c, ê→e, õ→o…).
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

pub struct Results<'a> {
    pub settings: Vec<&'a Entry>,
    pub actions: Vec<&'a Entry>,
    pub native: Vec<&'a Entry>,
}

/// Sinônimos da seção, usados só no último nível do ranking.
fn section_keywords(section: Section) -> &'static str {
    match section {
        Section::Power => "energia bateria cpu gpu watts desempenho economia perfil suspender ccd turbo",
        Section::Bar => "barra taskbar tray icones espacamento",
        Section::Monitor => "monitor esquerda cpu gpu temperatura ventoinha clima sensores processos",
        Section::Central => "central direita relogio calendario clipboard tema escuro nao perturbe",
        Section::Notifications => "notificacoes toast aviso pop-up som tempo posicao cor opacidade",
        Section::DevicesOsd => "osd logitech teclado mouse caps lock pilula aviso",
        Section::Camera => "camera audio microfone iriun webcam privacidade som",
        Section::Network => "rede wifi ethernet ip dns vpn bluetooth",
        Section::Security => "seguranca privacidade firewall ufw dns trackers mac vpn kernel usbguard",
        Section::Backup => "backup sincronizacao restic snapper btrfs ssd trim claude",
        Section::Displays => "monitores brilho escala ddc resolucao hz papel parede wallpaper janela dividida padding gap foco",
        Section::Login => "login bloqueio greeter senha avatar fundo",
        Section::Optional => "pacotes opcionais instalar apps extras",
        Section::System => "sistema servicos atualizacoes systemd daemon logs",
        _ => "",
    }
}

/// 0 = rótulo começa com a busca; 1 = palavra do rótulo começa; 2 = contém no rótulo;
/// 3 = contém em grupo, keywords ou sinônimos da seção.
fn rank(entry: &Entry, query: &str) -> Option<u8> {
    let label = normalize(entry.label);
    if label.starts_with(query) {
        return Some(0);
    }
    if label.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(query)) {
        return Some(1);
    }
    if label.contains(query) {
        return Some(2);
    }
    let extra = format!("{} {} {}", entry.group, entry.keywords, section_keywords(entry.section));
    normalize(&extra).contains(query).then_some(3)
}

fn top(index: &'static [Entry], query: &str, kind: EntryKind, max: usize) -> Vec<&'static Entry> {
    let mut hits: Vec<(u8, usize)> = index
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == kind)
        .filter_map(|(i, e)| rank(e, query).map(|r| (r, i)))
        .collect();
    // Empate: ordem do INDEX (o índice é a segunda chave).
    hits.sort_unstable();
    hits.into_iter().take(max).map(|(_, i)| &index[i]).collect()
}

/// Busca na paleta: no máximo 5 ajustes, 3 ações e 3 páginas nativas.
pub fn search(query: &str) -> Results<'static> {
    let query = normalize(query.trim());
    if query.is_empty() {
        return Results { settings: Vec::new(), actions: Vec::new(), native: Vec::new() };
    }
    Results {
        settings: top(INDEX, &query, EntryKind::Setting, MAX_SETTINGS),
        actions: top(INDEX, &query, EntryKind::Action, MAX_ACTIONS),
        native: top(INDEX, &query, EntryKind::Native, MAX_NATIVE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accents_and_case_are_ignored() {
        assert_eq!(normalize("Énergia Ç TELA"), "energia c tela");
        let a = search("recórte");
        let b = search("RECORTE");
        assert!(!a.settings.is_empty() || !a.actions.is_empty());
        assert_eq!(
            a.settings.iter().map(|e| e.label).collect::<Vec<_>>(),
            b.settings.iter().map(|e| e.label).collect::<Vec<_>>()
        );
    }

    #[test]
    fn print_finds_capture() {
        assert!(search("print").settings.iter().all(|e| e.section == Section::Capture));
        assert!(!search("print").settings.is_empty());
    }

    #[test]
    fn limits_and_prefix_first() {
        let r = search("tela");
        assert!(r.settings.len() <= 5 && r.actions.len() <= 3 && r.native.len() <= 3);
        assert!(
            normalize(r.settings[0].label).starts_with("tela")
                || normalize(r.settings[0].label).split(' ').any(|w| w.starts_with("tela"))
        );
        assert!(r.native.iter().any(|e| e.native_page == Some("power")));
    }

    #[test]
    fn empty_query_returns_nothing() {
        let r = search("  ");
        assert!(r.settings.is_empty() && r.actions.is_empty() && r.native.is_empty());
    }

    #[test]
    fn every_section_except_home_is_indexed() {
        for s in Section::ALL {
            if s != Section::Home {
                assert!(INDEX.iter().any(|e| e.section == s), "{s:?}");
            }
        }
    }

    #[test]
    fn battery_group_and_sysrq_are_indexed() {
        let r = search("bateria");
        assert!(
            r.settings.iter().any(|e| e.section == Section::Power && e.group == "Bateria"),
            "a busca por 'bateria' deve achar o grupo Bateria de Energia"
        );
        let r = search("sysrq");
        assert!(r.settings.iter().any(|e| e.section == Section::Security && e.label == "Desativar Magic SysRq"));
    }
}
