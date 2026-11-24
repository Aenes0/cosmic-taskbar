# Greeter: 5 estilos fiéis — Plano de implementação

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: superpowers:subagent-driven-development, conduzida pelo
> **controlador** da fábrica (protocolo `~/.claude/docs/fabrica/protocolo-controlador.md`). Passos com
> checkbox (`- [ ]`). **Nenhum passo de commit**: quem commita é o usuário.

**Goal:** os 5 estilos do aenes-greeter (bloqueio e login) ficam fiéis ao protótipo aprovado, com desfoque
real, tema claro de verdade, widgets com dados reais, usuário/sessão padrão no greeter e prévia do Painel
alinhada.

**Architecture:** `login_ui::screen_styled` vira despacho para `greeter/src/styles/<estilo>.rs`, que recebem
um `ScreenInput` com peças cruas (usuário, campo, energia, menus, dados do sistema, análise do fundo) e
montam a tela com `styles/parts.rs` (vidro, pílula, botões) e `styles/palette.rs` (cores, contraste, fontes).
`backdrop.rs` desfoca/analisa o papel de parede fora da thread de UI com cache em disco; `sysinfo.rs` +
UPower/NetworkManager entregam baterias e rede; `selection.rs` escolhe usuário/sessão padrão.

**Tech Stack:** Rust 2024, libcosmic/iced (checkout `libcosmic-41009aea1d72760b/430f3d4`), `image` 0.25.10
(`imageops::fast_blur`, `resize_to_fill`), zbus 5, `upower_dbus` e `cosmic-dbus-networkmanager`
(dbus-settings-bindings `eed01dd`), bash (`apply-greeter.sh`).

**Spec:** `docs/superpowers/specs/2026-11-24-greeter-estilos-fieis-design.md` (protótipo:
`.superpowers/brainstorm/195761-1795523755/content/estilos-v1.html` — todo trabalhador de estilo lê a seção
do seu estilo no CSS do protótipo).

## Linha de base medida (2026-11-24, HEAD `a471cdc` + working tree)

- Raiz: `flatpak-spawn --host sh -c 'cd ~/Projetos/cosmic-taskbar && cargo test --release'` →
  `test result: ok. 1039 passed; 0 failed; 5 ignored`.
- Greeter: `flatpak-spawn --host sh -c 'cd ~/Projetos/cosmic-taskbar/greeter && cargo test --release'` →
  **não compila**: 13 erros (E0277 `(): cosmic::iced::advanced::Renderer` não satisfeito; E0599 `layout`)
  em `src/offset.rs:190-213`. Há 87 atributos `#[test]` em `greeter/src/` (contagem estática). A linha de
  base real do greeter é a saída da Task 0; toda verificação de greeter antes dela é só `cargo build`.

## Global Constraints

- **Pré-condição do worktree:** o working tree tem a correção PAM **não commitada** (`greeter/src/locker.rs`:
  `resolve_pam_service`, `FALLBACK_PAM_SERVICE`, `PAM_DIRS`, 5 testes `pam_service_*`; `gaps.md`). O
  worktree da entrega tem de nascer com ela (o usuário commita antes, ou o Diretor aplica o `git diff`
  no worktree). Sem ela a senha volta a ser recusada.
- Nenhuma alteração em `pam_thread`, `resolve_pam_service`, `pam_service_from` nem no fluxo greetd de
  autenticação; "o bug de PAM de 24/11 não pode voltar".
- Contrato `~/.config/cosmic-taskbar/greeter.toml`: chaves inalteradas. Valores de `style`:
  `win11 | android | macos | ios | ipados`; `editorial` → `ios` e `sidebar` → `ipados` aceitos como
  sinônimos em `greeter/src/style_config.rs`, `src/control_panel/data.rs` e `extras/greeter/apply-greeter.sh`;
  o Painel grava só os novos.
- Boot lê o snapshot `/var/lib/aenes-greeter/greeter.toml` como usuário `cosmic-greeter`.
- O desfoque nunca atrasa a primeira pintura: sem resultado pronto, mostra a imagem sem desfoque e troca quando
  chegar. Nada de decodificar/desfocar/hashear imagem na thread de UI.
- Sem CPU contínuo com a tela parada: nenhuma assinatura nova de timer; a rotação usa o `Tick` de minuto que já
  existe; timer de 1 s só quando o estilo mostra segundos (`android` nunca).
- Legibilidade: contraste ≥ **4.5:1** (WCAG AA) para texto do campo (valor e placeholder) e da data, em
  qualquer tema × estilo × fundo claro/escuro.
- `card_opacity = 0` (≤ 0.01) = sem cartão em todos os estilos; sem mínimos por estilo; raio = `card_radius`.
- `blur_radius = 0` respeitado em todos os estilos; `BgSource::Color` não é desfocado.
- Fontes livres só quando `font_family = "system"`: win11 → Inter (Semibold no relógio); android → Nunito;
  macos → Nunito (pesos altos); ios/ipados → Inter (ExtraBold no relógio). `sans`/`mono`/`serif` sobrepõem.
  Fonte ausente cai para a do sistema sem erro. `install.sh` só imprime `sudo pacman -S inter-font ttf-nunito`.
- Sem logotipos, fontes ou imagens proprietárias; sem clima, notificações ou impressão digital; widget sem
  dado não aparece.
- Não alterar `/usr/bin/*` nem o pacote oficial `cosmic-greeter`.
- Faillock `deny=3`: em testes manuais, no máximo 1 senha errada por rodada.
- Shell do agente é sandbox Flatpak: `cargo`, `git`, `grim`, `cosmic-comp` só via
  `flatpak-spawn --host sh -c '...'`. Prints em `greeter/target/prints/` (caminho compartilhado).
- **Não commitar, não `git stash`, não `git reset`.** Fim de task = "Conferir diff (`git diff --stat`)".
- Arquivos novos de estilo ≤ 400 linhas cada; `login_ui.rs` não cresce.
- Licença de corrigir o plano (vale para todo trabalhador): se o código/assinatura do brief não compilar,
  contradisser as assinaturas reais ou falhar nos testes do próprio brief, o plano pode estar errado —
  corrija para satisfazer o teste e relate a divergência; nunca altere o teste para acomodar implementação
  defeituosa.

## Review Focus

1. **Papel de parede ruim** (bytes corrompidos, 8K, formato sem decoder, arquivo sumiu entre a escolha e o
   job): a tela abre com a imagem original ou o fundo de reserva, sem pânico; o job devolve `Err` e nada é
   trocado. → Task 7 `prepare_rejects_garbage_without_panic`, `reduced_size_caps_8k_and_ultrawide`.
2. **Usuário/sessão configurados que não existem**, diretório de sessões vazio, `last_user` de uid removido:
   pré-seleção cai para o próximo critério e `StartSession` nunca entra em pânico. → Task 10
   `pick_session_*`, `pick_user_*`.
3. **Monitores diferentes ao mesmo tempo** (ultrawide 5120×1440 + 1920×1080, 3 monitores): chave, cache e
   análise por monitor; o menu de cada subsuperfície usa a análise do seu monitor. → Task 7
   `key_differs_per_monitor_size`; Task 17/18 passo manual com 3 monitores.
4. **Cache inacessível ou inválido** (`/var/lib/aenes-greeter/cache` inexistente no boot, sem permissão,
   arquivo truncado): calcula sem cache, sobrescreve o inválido, sem erro em laço no journal. → Task 7
   `prepare_without_writable_cache_still_ok`, `truncated_cache_is_recomputed`.
5. **Config antiga ou mista** (`style = "editorial"` no snapshot gerado pelo script antigo; Painel antigo
   gravando `sidebar`): os três parsers mapeiam para o novo nome. → Task 2, Task 3, Task 4
   `script_maps_old_style_names`.

---

## Mapa de arquivos

| Arquivo | Responsabilidade | Tasks |
|---|---|---|
| `greeter/src/offset.rs` (testes 179-220) | só o módulo de testes | 0 |
| `src/control_panel/data.rs` | aliases + lista canônica de estilos | 3 |
| `extras/greeter/apply-greeter.sh`, `extras/greeter/greeter.toml.example`, `tests/apply_greeter.sh` (novo), `install.sh` | script root, fontes | 4 |
| `greeter/scripts/dev/*` (novos), `greeter/examples/server.rs` (novo) | harness aninhado | 5 |
| `greeter/src/lib.rs`, `login_ui.rs`, `time.rs`, `common.rs` (campo), `locker.rs`/`greeter.rs` (`menu`), `styles/mod.rs`, `styles/{win11,android,macos,ios,ipados}.rs`, stubs `styles/{palette,parts}.rs`, `backdrop.rs`, `sysinfo.rs`, `selection.rs`, `i18n/{en,pt-BR}/cosmic_greeter.ftl` | contrato e despacho | 1 |
| `greeter/src/style_config.rs` | aliases, campos novos, rotação, override do harness | 2 |
| `greeter/src/styles/palette.rs` | paleta, contraste, véu, tonal, fontes | 6 |
| `greeter/src/backdrop.rs` | blur, cache, análise, `BackdropSet` | 7 |
| `greeter/src/sysinfo.rs`, `upower.rs`, `networkmanager.rs`, `common.rs` | dados de bateria/rede | 8 |
| `greeter/src/styles/parts.rs` | primitivas visuais | 9 |
| `greeter/src/selection.rs`, `greeter.rs` (init, `start_session`, `Username`) | usuário/sessão padrão, `todo!()` | 10 |
| `src/control_panel/views.rs` (4049-4400) | prévia e lista de estilos do Painel | 11 |
| `greeter/src/styles/win11.rs` … `ipados.rs` | um estilo cada | 12-16 |
| `greeter/src/locker.rs` | ligação backdrop/rotação/sysinfo no bloqueio | 17 |
| `greeter/src/greeter.rs` | ligação backdrop/rotação/sysinfo no login | 18 |
| `greeter/src/login_ui.rs`, `styles/mod.rs` | remover adaptador legado | 19 |
| — (só prints) | verificação visual | 20 |

## Ondas

| Onda | Tasks (arquivos disjuntos dentro da onda) |
|---|---|
| 1 | 0, 3, 4, 5 — a Task 0 termina antes de qualquer `cargo test` do greeter nas ondas seguintes |
| 2 | 1, 2 |
| 3 | 6, 7, 8, 9, 10, 11 |
| 4 | 12, 13, 14, 15, 16, 17, 18 |
| 5 | 19 |
| 6 | 20 |

Tasks **[RISCO]**: 1 (primeira pintura do bloqueio, `menu` do locker e do greeter), 4 (script root),
10 (`todo!()`/sessão do greeter), 17 (primeira pintura do bloqueio), 18 (primeira pintura do login/boot).

Verificação padrão (o controlador roda depois de cada task):
- greeter: `flatpak-spawn --host sh -c 'cd ~/Projetos/cosmic-taskbar/greeter && cargo test --release 2>&1 | grep -E "^test result|FAILED|panicked|^error"'`
- raiz: `flatpak-spawn --host sh -c 'cd ~/Projetos/cosmic-taskbar && cargo test --release 2>&1 | grep -E "^test result|FAILED|^error"'`

(Os caminhos acima são do checkout principal; no worktree, troque pelo caminho do worktree.)

---

### Task 0: Testes do `offset.rs` voltam a compilar

**Onda 1 · depende de: — · trabalhador mínimo: `sonnet medium`**

**Files:**
- Modify: `greeter/src/offset.rs:179-220` (só `mod tests`)

**Interfaces:** nenhuma; código de produção (`offset.rs:1-177`) intocado.

Contexto conferido: os testes usam `Offset<'_, (), (), ()>`, `Tree::new(&w as &dyn Widget<(), (), ()>)` e
`w.layout(&mut tree, &(), …)`. No libcosmic `430f3d4`, `()` não satisfaz `cosmic::iced::advanced::Renderer`
(o compilador aponta `iced/tiny_skia/src/lib.rs:202`, `impl core::Renderer for Renderer`). Caminhos
possíveis: `cosmic::Renderer` (= `iced::Renderer`, `src/lib.rs:183` do libcosmic) ou
`iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(16.0))` (`tiny_skia/src/lib.rs:53`), Theme
`cosmic::Theme`. O trabalhador escolhe o que compila sem dependência nova.

- [ ] **Passo 1:** rodar a suíte do greeter e confirmar os 13 erros em `src/offset.rs:190-213`.
- [ ] **Passo 2:** trocar Message/Theme/Renderer dos 3 testes (`offset_moves_child_but_keeps_own_size`,
  `offset_tree_is_transparent`, `offset_tree_is_transparent_for_stateful_child`) por tipos que compilam;
  **asserções idênticas** (mesmos valores: `Size::new(10.0, 20.0)`, `Rectangle::new([3.0, 5.0].into(), …)`,
  tags e contagem de filhos).
- [ ] **Passo 3:** suíte do greeter → compila e passa; anotar a contagem (`N passed`) no ledger como
  **linha de base do greeter**.
- [ ] **Passo 4:** Conferir diff (`git diff --stat` → só `greeter/src/offset.rs`).

---

### Task 1: Contrato do despacho e mudança dos 5 ramos para `styles/` [RISCO]

**Onda 2 · depende de: 0 · trabalhador mínimo: `sonnet high`**

Refatoração **sem mudança visual**: o código atual de cada ramo de `screen_styled` (`login_ui.rs:1051-1385`)
muda para o arquivo do estilo, alimentado por um adaptador; locker e greeter passam a montar `ScreenInput`.

**Files:**
- Modify: `greeter/src/lib.rs` (declarar `mod styles; mod backdrop; mod sysinfo; mod selection;`)
- Modify: `greeter/src/login_ui.rs:316-1385` (helpers usados pelos estilos viram `pub(crate)`:
  `shadow_text`, `shadow_text_aligned`, `shift`, `white`, `black`, `clock_font`, `date_font`,
  `assemble_login_column`, `maybe_glass_card`, `screen`, constantes de tamanho; `screen_styled` removida)
- Modify: `greeter/src/time.rs` (novo `format_date_short`)
- Modify: `greeter/src/common.rs:45-76,97-140` (campo `pub system_info: crate::sysinfo::SystemInfo`, `Default`)
- Modify: `greeter/src/locker.rs:633-748` (`menu`) — **não tocar** `pam_thread`/`resolve_pam_service`
- Modify: `greeter/src/greeter.rs:622-753` (`menu`)
- Modify: `greeter/i18n/en/cosmic_greeter.ftl`, `greeter/i18n/pt-BR/cosmic_greeter.ftl`
- Create: `greeter/src/styles/mod.rs`, `styles/win11.rs`, `android.rs`, `macos.rs`, `ios.rs` (ramo
  `editorial`), `ipados.rs` (ramo `sidebar`)
- Create (só tipos): `greeter/src/styles/palette.rs`, `greeter/src/backdrop.rs`, `greeter/src/sysinfo.rs`
- Create (só doc `//!`): `greeter/src/styles/parts.rs`, `greeter/src/selection.rs`

**Interfaces — Produces** (nomes exatos; as ondas 3-4 dependem deles):

```rust
// styles/mod.rs
pub struct UserParts<'a> { pub icon: Option<&'a widget::image::Handle>, pub full_name: &'a str }
pub struct FieldParts<'a, M> {            // input já com id, on_input, on_submit, on_focus/on_unfocus;
    pub input: widget::TextInput<'a, M>,  // sem padding/largura/estilo (o estilo decide)
    pub submit: Option<M>,
    pub fx: login_ui::FieldFx<M>,
}
pub struct PowerParts<M> { pub open: bool, pub on_toggle: M,
    pub on_action: fn(crate::power::PowerAction) -> M, pub fx: login_ui::PowerFx<M> }
pub struct ChoiceMenu<M> { pub open: bool, pub on_toggle: M, pub label: String,
    pub items: Vec<(String, bool, M)> }   // (rótulo, marcado, mensagem)
pub struct Looks { pub clock: Look, pub avatar: Look, pub name: Look,
    pub field: Look, pub status: Look, pub power: Look }
pub struct ScreenInput<'a, M> {
    pub cfg: &'a StyleConfig,
    pub metrics: login_ui::ScreenMetrics,
    pub clock: login_ui::ClockData,          // ganha `short_date: String`
    pub user: UserParts<'a>,
    pub field: Option<FieldParts<'a, M>>,
    pub status: login_ui::StatusLine<'a>,
    pub looks: Looks,
    pub power: Option<PowerParts<M>>,
    pub session: Option<ChoiceMenu<M>>,      // só o greeter (login)
    pub keyboard: Option<ChoiceMenu<M>>,     // só com 2+ layouts ativos
    pub system: &'a crate::sysinfo::SystemInfo,
    pub backdrop: Option<&'a crate::backdrop::Analysis>,
}
pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M>; // despacho por cfg.style
pub(crate) fn legacy_parts<'a, M: Clone + 'static>(input: ScreenInput<'a, M>)
    -> (login_ui::LoginBlock<'a, M>, Option<Element<'a, M>>, login_ui::ClockData); // adaptador temporário

// cada styles/<estilo>.rs
pub fn view<'a, M: Clone + 'static>(input: ScreenInput<'a, M>) -> Element<'a, M>;

// styles/palette.rs (só o tipo; funções na Task 6)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette { pub text: Color, pub text_muted: Color, pub text_shadow: Color,
    pub field_fill: Color, pub field_text: Color, pub field_placeholder: Color, pub field_border: Color,
    pub glass_fill: Color, pub glass_border: Color, pub glass_shadow: Color,
    pub button_fill: Color, pub button_icon: Color, pub card_fill: Color, pub veil: Color,
    pub accent: Option<Color> }

// backdrop.rs (só tipos; funções na Task 7)
pub const GRID: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Region { pub x0: f32, pub y0: f32, pub x1: f32, pub y1: f32 } // 0..=1
#[derive(Clone, Debug, PartialEq)] pub struct Analysis {
    pub luminance: [[f32; GRID]; GRID],   // [linha][coluna], luminância relativa WCAG 0..=1, já em "cover"
    pub average: [f32; 3] }               // sRGB 0..=1

// sysinfo.rs (só tipos + Default; métodos na Task 8)
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum DeviceKind { System, Mouse, Keyboard, Headset, Gamepad, Phone, Tablet, Other }
#[derive(Clone, Debug, PartialEq)] pub struct BatteryDevice { pub kind: DeviceKind, pub name: String, pub percent: u8, pub charging: bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum NetworkKind { Wired, Wireless { strength: u8 } }
#[derive(Clone, Debug, PartialEq)] pub struct NetworkInfo { pub kind: NetworkKind, pub name: String }
#[derive(Clone, Debug, Default, PartialEq)] pub struct SystemInfo { pub batteries: Vec<BatteryDevice>, pub network: Option<NetworkInfo> }

// time.rs
pub fn format_date_short(&self) -> String;   // "ter., 24 de nov." (fieldsets::MDE::medium ou equivalente)
```

Assinaturas reais conferidas que o adaptador usa: `login_ui::avatar(icon, size, look)` (`login_ui.rs:378`),
`user_name_styled(full_name, look, cfg)` (`:875`), `password_row_styled(input, submit, fx, cfg)` (`:541`),
`status_view_styled(status, look, cfg)` (`:897`), `power_button(open, on_toggle, on_action, fx)` (`:749`),
`ClockData` (`:929-935`), `LoginBlock` (`:938-944`).

Chaves i18n novas (en / pt-BR; se já existir, reutilize): `sign-in-options` (Sign-in options / Opções de
entrada), `unlock-hint` (Enter your password to unlock / Digite a senha para desbloquear), `session-not-found`
(No session available / Nenhuma sessão disponível), `battery-system` (Battery / Bateria), `battery-mouse`,
`battery-keyboard`, `battery-headset`, `battery-gamepad`, `battery-device` (Mouse/Teclado/Fone/Controle/
Dispositivo), `network` (Network / Rede), `keyboard-layout` (Keyboard layout / Layout do teclado).

- [ ] **Passo 1 (teste que falha):** em `styles/mod.rs`, `#[test] fn dispatch_names_are_canonical()` —
  `assert_eq!(STYLE_NAMES, ["win11", "android", "macos", "ios", "ipados"]);` (constante pública
  `pub const STYLE_NAMES: [&str; 5]` usada pelo despacho; estilo desconhecido → `win11`).
- [ ] **Passo 2:** suíte do greeter → falha (módulo não existe).
- [ ] **Passo 3:** criar os módulos/tipos acima; mover cada ramo para seu arquivo chamando `legacy_parts`;
  `locker.rs:menu` e `greeter.rs:menu` montam `ScreenInput` (`session: None` e `keyboard: None` por
  enquanto; `system: &self.common.system_info`; `backdrop: None`) e chamam `styles::view`.
- [ ] **Passo 4:** suíte do greeter verde (linha de base da Task 0 + 1); `cargo build --release` sem warning
  novo de código morto nos tipos (use `#[allow(dead_code)]` só nos tipos que as ondas 3-4 consomem, com
  comentário "usado a partir da onda 3").
- [ ] **Passo 5 (visual, controlador):** Task 5 já trouxe o harness: `scripts/dev/nested-locker.sh` +
  `print.py antes-depois-win11` → idêntico ao bloqueio atual (mesmo estilo; nada de tela vazia).
- [ ] **Passo 6:** Conferir diff (`git diff --stat`).

---

### Task 2: `style_config.rs` — aliases, usuário/sessão, rotação, override do harness

**Onda 2 · depende de: 0 · trabalhador mínimo: `sonnet medium`**

**Files:**
- Modify: `greeter/src/style_config.rs:19-60` (struct), `:62-87` (Default), `:103-225` (parse),
  `:229-260` (escolha), `:350-359` (`candidate_config_paths`), testes `:492+`

**Interfaces — Produces:**
```rust
pub struct StyleConfig { /* campos atuais */ pub default_user: String, pub session: String } // defaults ""
pub fn canonical_style(raw: &str) -> Option<&'static str>; // win11|android|macos|ios|ipados; editorial→ios; sidebar→ipados
impl StyleConfig {
    pub fn rotation_slot(&self, now_unix_secs: u64) -> Option<u64>;        // Some(now/period) só em rotate_interval
    pub fn pick_wallpaper_login_at(&self, now_unix_secs: u64) -> Option<PathBuf>; // rotate_on_lock usa o horário
    pub fn needs_second_tick(&self) -> bool;                              // show_seconds && style != "android"
}
pub fn rotation_due(last: Option<u64>, now: Option<u64>) -> bool;        // true só se ambos Some e diferentes
pub fn wants_keyboard_menu(active_layouts: usize) -> bool;               // >= 2
pub const CONFIG_OVERRIDE_ENV: &str = "AENES_GREETER_CONFIG";
fn candidate_config_paths_from(nested: bool, override_path: Option<&OsStr>,
    xdg: Option<&OsStr>, home: Option<PathBuf>) -> Vec<PathBuf>;           // override só se nested
```
`candidate_config_paths()` passa a chamar `candidate_config_paths_from(crate::power::nested_from_env(), …)`.

- [ ] **Passo 1 (testes que falham):**
```rust
#[test] fn style_aliases_map_to_new_names() {
    assert_eq!(StyleConfig::parse("style = \"editorial\"").style, "ios");
    assert_eq!(StyleConfig::parse("style = \"sidebar\"").style, "ipados");
    assert_eq!(StyleConfig::parse("style = \"IPADOS\"").style, "ipados");
    assert_eq!(StyleConfig::parse("style = \"vista\"").style, "win11");
}
#[test] fn parses_default_user_and_session() {
    let c = StyleConfig::parse("default_user = \"maria\"\nsession = \"COSMIC\"");
    assert_eq!((c.default_user.as_str(), c.session.as_str()), ("maria", "COSMIC"));
    assert_eq!(StyleConfig::default().session, "");
}
#[test] fn rotation_slot_only_for_interval() {
    let mut c = StyleConfig::default(); c.wallpaper_interval_mins = 10;
    assert_eq!(c.rotation_slot(1200), None);
    c.wallpaper_mode = "rotate_interval".into();
    assert_eq!(c.rotation_slot(599), Some(0)); assert_eq!(c.rotation_slot(600), Some(1));
}
#[test] fn login_rotate_on_lock_picks_by_time() { /* dir temp a,b,c sem shuffle, intervalo 10:
    pick_wallpaper_login_at(0)==a, (600)==b, (1200)==c, mesmo com wallpaper_mode = "rotate_on_lock" */ }
#[test] fn rotation_due_only_on_slot_change() {
    assert!(rotation_due(Some(1), Some(2))); assert!(!rotation_due(Some(2), Some(2)));
    assert!(!rotation_due(None, None)); assert!(!rotation_due(Some(1), None));
    assert!(!wants_keyboard_menu(1)); assert!(wants_keyboard_menu(2));
}
#[test] fn second_tick_never_on_android() {
    let mut c = StyleConfig::default(); c.show_seconds = true;
    assert!(c.needs_second_tick()); c.style = "android".into(); assert!(!c.needs_second_tick());
}
#[test] fn override_path_only_when_nested() {
    let p = candidate_config_paths_from(true, Some(OsStr::new("/tmp/x.toml")), None, Some("/h".into()));
    assert_eq!(p[0], PathBuf::from("/tmp/x.toml"));
    let q = candidate_config_paths_from(false, Some(OsStr::new("/tmp/x.toml")), None, Some("/h".into()));
    assert!(!q.contains(&PathBuf::from("/tmp/x.toml")));
}
```
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar; `pick_wallpaper_at` atual continua igual para o bloqueio.
- [ ] **Passo 4:** suíte do greeter verde (testes existentes de `pick_wallpaper_fixed_and_rotate_modes`
  inalterados).
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `style_config.rs`).

---

### Task 3: Painel — aliases e lista canônica no `data.rs`

**Onda 1 · depende de: — · trabalhador mínimo: `sonnet medium`**

**Files:**
- Modify: `src/control_panel/data.rs:139` (doc), `:292-296` (parse de `style`), testes `:1220-1282`

**Interfaces — Produces:**
```rust
/// (id gravado, nome no Painel, descrição curta) — ordem da lista do Painel.
pub const GREETER_STYLES: [(&str, &str, &str); 5] = [
    ("win11", "Windows 11", …), ("android", "Android 16 (Pixel)", …), ("macos", "macOS Tahoe", …),
    ("ios", "iOS 26", …), ("ipados", "iPadOS 26", …) ];
pub fn canonical_greeter_style(raw: &str) -> Option<&'static str>; // mesmos aliases da Task 2
```
Descrições: frase curta tirada da tabela do item 1 da spec (o trabalhador redige; sem nome de produto além do
título).

- [ ] **Passo 1 (testes que falham):** `greeter_style_aliases` — `parse_greeter_toml("style = \"editorial\"").style == "ios"`,
  `"sidebar"` → `"ipados"`, `"Android"` → `"android"`, `"xyz"` → `"win11"`; `greeter_styles_round_trip` —
  para cada id de `GREETER_STYLES`, `parse_greeter_toml(&format_greeter_toml(&cfg_com_esse_id)).style == id`.
- [ ] **Passo 2:** suíte da raiz → falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte da raiz: 1039 + 2 passed, 0 failed.
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `data.rs`).

---

### Task 4: `apply-greeter.sh` sem blur/dim nem `last_user`, cache do boot, fontes no `install.sh` [RISCO]

**Onda 1 · depende de: — · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `extras/greeter/apply-greeter.sh:34-57` (defaults), `:271-276` (style), `:581-671`
  (`prepare_wallpaper`), `:673-749` (`write_greeter_cosmic_config`), `:751+` (snapshot), `:903` (guarda de
  biblioteca antes de `# --- Execução principal ---`)
- Modify: `extras/greeter/greeter.toml.example` (valores de `style` documentados)
- Modify: `install.sh:47-55` (aviso de fontes após instalar o greeter)
- Create: `tests/apply_greeter.sh` (padrão de `tests/setup_corner.sh`)

Regras:
- Defaults iguais ao parser (`style_config.rs:62-87`): `CFG_FONT_WEIGHT="normal"`, `CFG_CLOCK_SIZE="medium"`,
  `CFG_CARD_OPACITY="0.0"`, `CFG_CARD_RADIUS="16"`; `CFG_SESSION=""` (vazio = última sessão no greeter).
- `style`: `win11|android|macos|ios|ipados` → como está; `editorial` → `ios`; `sidebar` → `ipados`; outro → `win11`.
- `prepare_wallpaper`: só copia o original (`install -Dm0644 "$src" "$STATE_DIR/wallpaper.$ext"`); remover os
  ramos Python/ImageMagick e `need_fx`.
- `write_greeter_cosmic_config`: deixa de gravar `com.system76.CosmicGreeter/v1/last_user` e `users`; mantém o
  aviso de `default_user` inexistente; não apaga arquivos antigos.
- Criar `$STATE_DIR/cache` com `install -dm0700 -o "$GREETER_USER" -g "$GREETER_USER"` (também no dry-run: só log).
- Guarda: `[ "${APPLY_GREETER_LIB:-0}" = 1 ] && return 0 2>/dev/null` imediatamente antes da execução principal,
  para os testes carregarem as funções sem root.
- `install.sh`: depois de instalar `aenes-greeter`, se `fc-list : family | grep -qi '^Inter'` ou `Nunito` falhar,
  imprimir exatamente `Fontes do greeter ausentes: sudo pacman -S inter-font ttf-nunito` (não instala nada).

- [ ] **Passo 1 (teste que falha) — `tests/apply_greeter.sh`:** carrega o script com `APPLY_GREETER_LIB=1`,
  `STATE_DIR` e `get_user_home` redefinidos para um `mktemp -d`, `GREETER_USER=$(id -un)`; casos:
  `script_maps_old_style_names` (`style = "editorial"` → `CFG_STYLE=ios`; `"sidebar"` → `ipados`;
  `"macos"` → `macos`); `defaults_match_parser` (sem arquivo: `CFG_CARD_OPACITY=0.0`,
  `CFG_FONT_WEIGHT=normal`, `CFG_CLOCK_SIZE=medium`, `CFG_CARD_RADIUS=16`);
  `wallpaper_copied_verbatim` (com `blur_radius = 20`, `dim_opacity = 0.5`, `cmp` original vs
  `$STATE_DIR/wallpaper.png` idênticos); `no_last_user_written` (após `write_greeter_cosmic_config` com
  `default_user` = usuário atual, não existem `…/CosmicGreeter/v1/last_user` nem `users`).
- [ ] **Passo 2:** `flatpak-spawn --host sh -c 'cd ~/Projetos/cosmic-taskbar && sh tests/apply_greeter.sh'` → FALHA.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** `sh tests/apply_greeter.sh` → todos OK; `bash -n extras/greeter/apply-greeter.sh`;
  `shellcheck` se existir no host (sem aviso novo); `sh tests/setup_corner.sh` continua OK.
- [ ] **Passo 5:** **não** rodar o script com root nem `--rebuild`. Só `--dry-run --config <toml de teste>`
  como usuário comum se o script permitir; senão, ficar nos testes.
- [ ] **Passo 6:** Conferir diff (`git diff --stat`).

---

### Task 5: Harness aninhado no greeter (`scripts/dev`, `examples/server.rs`)

**Onda 1 · depende de: — · trabalhador mínimo: `sonnet medium`**

**Files:**
- Create: `greeter/scripts/dev/{nested-locker.sh,nested-greeter.sh,print.py,stop-nested.sh}` a partir de
  `~/Projetos/cosmic-greeter/scripts/dev/` (39, 35, 32, 45 linhas; ler no host)
- Create: `greeter/scripts/dev/prints-estilos.sh`, `greeter/scripts/dev/nested-panel.sh`
- Create: `greeter/examples/server.rs` (cópia de `~/Projetos/cosmic-greeter/examples/server.rs`, 82 linhas;
  greetd falso: senha `password` passa; usa `greetd_ipc` e `tokio`, já dependências do greeter)

Regras:
- `repo` dos scripts = `greeter/` (`$(dirname "$0")/../..`); binários `greeter/target/release/cosmic-greeter`
  e `greeter/target/release/examples/server`.
- `nested-locker.sh`/`nested-greeter.sh` repassam `AENES_GREETER_CONFIG` ao cliente (como já fazem com
  `COSMIC_GREETER_PERF_LOG`).
- `prints-estilos.sh [locker|greeter]`: para cada `style` em `win11 android macos ios ipados` × `theme` em
  `dark light`, grava `target/dev/cfg/<style>-<theme>.toml` (copiando o `greeter.toml` do usuário e trocando
  só `style`/`theme`), abre o aninhado com `AENES_GREETER_CONFIG` apontando para ele, espera
  `target/dev/nested.display` + 3 s, `print.py <modo>-<style>-<theme>`, `stop-nested.sh`. Nunca digita senha.
- `nested-panel.sh <style> <theme>`: `XDG_CONFIG_HOME` temporário em `target/dev/xdg-<style>-<theme>/` com
  `cosmic-taskbar/greeter.toml`, roda `../target/release/cosmic-taskbar --control-panel --section login` no
  cosmic-comp aninhado; `stop-nested.sh` passa a reconhecer esse binário (regex ancorada, pidfiles).
- Nunca `pkill cosmic-comp` (mata a sessão real); só pidfiles, como o `stop-nested.sh` original.

- [ ] **Passo 1:** `cargo build --release --example server` no greeter → compila.
- [ ] **Passo 2:** `sh -n` em todos os `.sh`; `python3 -m py_compile scripts/dev/print.py`.
- [ ] **Passo 3 (controlador):** `scripts/dev/nested-locker.sh &` → `print.py teste-harness` gera
  `greeter/target/prints/teste-harness.png`; `stop-nested.sh` encerra só o aninhado (conferir
  `pgrep -a cosmic-comp` antes/depois: o da sessão real continua).
- [ ] **Passo 4:** Conferir diff (`git status --short greeter/scripts greeter/examples`).

---

### Task 6: Paleta, contraste, véu, cores tonais e fontes

**Onda 3 · depende de: 1, 2 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/styles/palette.rs` (tipo `Palette` da Task 1 + funções e testes)

**Interfaces — Consumes:** `Palette` (Task 1), `StyleConfig` com estilos canônicos (Task 2).
**Produces:**
```rust
pub const MIN_CONTRAST: f32 = 4.5;
pub const MAX_VEIL: f32 = 0.92;
pub fn relative_luminance(c: Color) -> f32;              // WCAG, ignora alpha
pub fn contrast_ratio(l1: f32, l2: f32) -> f32;          // (max+0.05)/(min+0.05)
pub fn composite_luminance(over: Color, bg_lum: f32) -> f32; // mistura sRGB de `over` (com alpha) sobre cinza de luminância bg_lum
pub fn min_alpha_for_contrast(text: Color, over: Color, bg_lum: f32, floor: f32) -> f32; // menor alpha em [floor, MAX_VEIL] com contraste ≥ MIN_CONTRAST (MAX_VEIL se nenhum)
pub fn palette(cfg: &StyleConfig, tonal_source: Option<[f32; 3]>) -> Palette; // tonal só no android; accent_color vence
pub enum FontRole { Clock, Date, Body, Name }
pub fn style_font(cfg: &StyleConfig, role: FontRole) -> Font;
```
Regras: `palette` muda **tudo** com `theme` (texto claro + véu preto no escuro; texto escuro + véu branco no
claro). Vidro do protótipo (escuro): preenchimento branco ~14 %, borda branca ~28 %; o `field_fill` é a base
e `parts`/estilos elevam o alpha com `min_alpha_for_contrast` sobre a luminância medida. Android: campo claro
tonal e botões escuros tonais derivados de `tonal_source` (sem lib de Material You), com luminosidade presa
para garantir o contraste. `style_font`: tabela de fontes das Global Constraints; `font_weight != "normal"`
sobrepõe só `FontRole::Clock`.

- [ ] **Passo 1 (testes que falham):**
```rust
#[test] fn contrast_reference_values() {
    assert!((contrast_ratio(1.0, 0.0) - 21.0).abs() < 0.01);
    assert!((relative_luminance(Color::WHITE) - 1.0).abs() < 1e-4);
}
#[test] fn field_text_readable_on_any_backdrop() {
    // 5 estilos × {dark, light} × bg_lum em 0.0..=1.0 passo 0.05, tonal None:
    // fill = Color { a: min_alpha_for_contrast(p.field_text, p.field_fill, bg, p.field_fill.a), ..p.field_fill };
    // contrast_ratio(relative_luminance(p.field_text), composite_luminance(fill, bg)) >= MIN_CONTRAST
    // e o mesmo para p.field_placeholder.
}
#[test] fn date_readable_after_veil() {
    // idem com p.text sobre p.veil: alpha = min_alpha_for_contrast(p.text, p.veil, bg, 0.0); contraste >= 4.5
}
#[test] fn android_tonal_field_has_contrast_for_any_wallpaper() {
    // tonal_source em {0, .25, .5, .75, 1}^3 × {dark, light}: campo opaco, contraste(field_text, field_fill) >= 4.5
}
#[test] fn accent_color_wins_over_tonal() { /* cfg.accent_color = "#60A5FA" → p.accent == Some(cor) */ }
#[test] fn light_theme_changes_whole_palette() { /* dark vs light: text, field_fill, glass_fill e button_icon diferentes */ }
#[test] fn fonts_per_style() {
    // win11+system, Clock → Font { family: Family::Name("Inter"), weight: Weight::Semibold, .. }
    // android+system, Date → family Name("Nunito"); ios+system, Clock → weight ExtraBold
    // ios + font_family "mono" → family Family::Monospace
    // win11 + font_weight "light": Clock → Light; Date → peso nativo inalterado
}
```
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte do greeter verde.
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `palette.rs`).

---

### Task 7: `backdrop.rs` — desfoque, cache, análise e conjunto por superfície

**Onda 3 · depende de: 1 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/backdrop.rs` (tipos da Task 1 + funções e testes)

**Interfaces — Consumes:** `Analysis`, `Region`, `GRID` (Task 1). **Produces:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackdropKey { pub hash: u64, pub radius: u32, pub width: u32, pub height: u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tag { pub image: widget::image::Id, pub radius: u32, pub monitor: (u32, u32) } // barato, sem hash dos bytes
#[derive(Clone, Debug)]
pub struct Prepared { pub tag: Tag, pub key: BackdropKey,
    pub blurred: Option<widget::image::Handle>,   // None quando radius == 0
    pub analysis: Analysis }
pub fn fnv1a64(bytes: &[u8]) -> u64;
pub fn reduced_size(monitor: (u32, u32)) -> (u32, u32);         // cada lado ÷ 4, arredonda p/ cima, mín. 1
pub fn key_for(bytes: &[u8], radius: u32, monitor: (u32, u32)) -> BackdropKey;
pub fn cache_file_name(key: &BackdropKey) -> String;            // "{hash:016x}-r{radius}-{w}x{h}.png"
pub fn cache_dir(boot: bool) -> Option<PathBuf>;                // boot: /var/lib/aenes-greeter/cache (se dir existir);
                                                                // senão $XDG_CACHE_HOME|~/.cache + "/aenes-greeter" (0700)
pub fn handle_bytes(handle: &widget::image::Handle) -> Option<Bytes>; // Bytes→clone barato; Path→None (não lê na UI); Rgba→None
pub fn analyze(img: &image::RgbaImage) -> Analysis;
pub fn prepare(bytes: &[u8], tag: Tag, cache: Option<&Path>) -> Result<Prepared, String>; // bloqueante
pub async fn prepare_async(bytes: Bytes, tag: Tag, cache: Option<PathBuf>) -> Result<Prepared, String>; // spawn_blocking
impl Analysis { pub fn region_luminance(&self, r: Region) -> f32; pub fn uniform(lum: f32, average: [f32; 3]) -> Self; }
pub fn surface_tag(image: &widget::image::Handle, radius: u32, size: Option<(u32, u32)>) -> Option<Tag>; // sem tamanho → None
pub fn pick_view_handle<'a>(blur: u32, prepared: Option<&'a Prepared>, tag: Tag,
    original: &'a widget::image::Handle) -> &'a widget::image::Handle; // blur 0 ou tag ≠ → original
pub struct BackdropSet<K> { /* pendentes e prontos por K */ }
impl<K: Copy + Eq + Hash> BackdropSet<K> {
    pub fn wants(&self, k: K, tag: Tag) -> bool;     // não pronto nem pendente com esse tag
    pub fn mark_pending(&mut self, k: K, tag: Tag);
    pub fn accept(&mut self, k: K, p: Prepared) -> bool; // false (descarta) se p.tag != pendente atual
    pub fn get(&self, k: K) -> Option<&Prepared>;
    pub fn forget(&mut self, k: K);
}
```
Algoritmo de `prepare`: calcula `key_for`; cache válido = arquivo existe, decodifica, dimensões = `(key.width,
key.height)` → usa sem decodificar o original. Senão: decodifica (`image::load_from_memory`), `resize_to_fill`
para `reduced_size` (filtro Triangle), `fast_blur(sigma = radius / 4)` se `radius > 0`, grava PNG em
`<dir>/<nome>.tmp` e renomeia; erro de escrita só vira `tracing::debug!` (segue sem cache). `analyze` sobre a
imagem reduzida (desfocada quando houver). `blurred` = `Handle::from_rgba`. Nada disso roda na thread de UI.

- [ ] **Passo 1 (testes que falham):** com imagem sintética 64×36 (metade esquerda branca, direita preta,
  codificada em PNG em memória):
  - `reduced_size_caps_8k_and_ultrawide`: `(7680,4320)→(1920,1080)`, `(5120,1440)→(1280,360)`, `(1,1)→(1,1)`.
  - `key_differs_per_monitor_size`: mesmos bytes, monitores `(1920,1080)` e `(5120,1440)` → chaves ≠;
    mesmo monitor e raio → `==`; raio 0 vs 20 → ≠.
  - `fnv_is_stable`: `fnv1a64(b"")==0xcbf29ce484222325`, `fnv1a64(b"a")==0xaf63dc4c8601ec8c`.
  - `analysis_sees_left_bright_right_dark`: `region_luminance(Region{x0:0.,y0:0.,x1:0.5,y1:1.}) > 0.8` e a
    direita `< 0.05`.
  - `cache_hit_skips_decoding`: `prepare(bons_bytes, raio 8)` grava o cache; substituir esse arquivo por um PNG
    válido, todo vermelho, com as dimensões da chave; `prepare(bons_bytes, raio 8)` de novo →
    `analysis.average` ≈ `[1.0, 0.0, 0.0]` (prova que leu o cache em vez de decodificar o original).
  - `truncated_cache_is_recomputed`: arquivo de cache com 10 bytes → `Ok`, e o arquivo passa a decodificar.
  - `prepare_without_writable_cache_still_ok`: `cache = Some("/proc/aenes-nao-existe")` → `Ok`.
  - `prepare_rejects_garbage_without_panic`: bytes `b"nao e imagem"` sem cache → `Err(_)`.
  - `radius_zero_has_no_blurred_handle`: `Prepared.blurred.is_none()` e `analysis` presente.
  - `surface_tag_tracks_inputs`: sem tamanho → `None`; mesma imagem/raio/tamanho → iguais; raio ou tamanho
    diferente → diferentes.
  - `view_handle_falls_back_to_original`: `pick_view_handle(0, Some(&p), p.tag, &orig)` → `orig`;
    tag diferente → `orig`; raio > 0, pronto e tag igual → `p.blurred`.
  - `set_discards_stale_result`: `mark_pending(k, tag_a)`, depois `mark_pending(k, tag_b)`;
    `accept(k, prepared_com_tag_a) == false`; `accept(k, …tag_b) == true`; `wants(k, tag_b) == false`.
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar (sem dependência nova; FNV-1a 64 local).
- [ ] **Passo 4:** suíte do greeter verde.
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `backdrop.rs`).

---

### Task 8: Dados reais de bateria e rede (`sysinfo.rs`, UPower, NetworkManager, `common.rs`)

**Onda 3 · depende de: 1 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/sysinfo.rs` (métodos e testes)
- Modify: `greeter/src/upower.rs:10-86` (nova assinatura de periféricos; a de bateria do sistema continua)
- Modify: `greeter/src/networkmanager.rs:11-105` (payload passa a `Option<NetworkInfo>`)
- Modify: `greeter/src/common.rs:79-94` (Message), `:290-292` (NetworkIcon), `:439-490` (subscription), helpers

**Interfaces — Consumes:** tipos de `sysinfo.rs` (Task 1). APIs conferidas: `UPowerProxy::enumerate_devices()
-> Vec<OwnedObjectPath>` (`upower/src/upower.rs:11`), sinais `device_added`/`device_removed` (`:22,26`),
`Device` com `type_()`, `percentage()`, `is_present()`, `power_supply()`, `model()`, `state()`
(`upower/src/device.rs:79-187`), `BatteryType` (`device.rs:22-41`); NM: `ActiveConnection` deref para proxy
com `id()` e `type_()` (`networkmanager/src/interface/active_connection.rs:56,84`), `AccessPoint` deref com
`ssid() -> Vec<u8>` (`interface/access_point.rs:60`), `NetworkManager::active_connections()` (`nm.rs:62`).
**Produces:**
```rust
// sysinfo.rs
impl DeviceKind { pub fn icon_name(self, percent: u8, charging: bool) -> &'static str; pub fn label_key(self) -> &'static str /* "battery-mouse" … */ }
impl NetworkInfo { pub fn icon_name(&self) -> &'static str }   // mesmos nomes de NetworkIcon::name (networkmanager.rs:19-34)
impl SystemInfo {
    pub fn glance_battery(&self) -> Option<&BatteryDevice>;   // System se houver; senão o periférico de menor %
    pub fn peripherals(&self) -> impl Iterator<Item = &BatteryDevice>; // kind != System, ordem estável por (kind, name)
}
pub fn sorted_batteries(v: Vec<BatteryDevice>) -> Vec<BatteryDevice>;
// upower.rs (feature "upower")
pub fn classify(t: BatteryType, power_supply: bool, is_present: bool, percentage: f64) -> Option<DeviceKind>;
pub fn devices_subscription() -> Subscription<Vec<BatteryDevice>>; // reenvia só quando a lista muda
// networkmanager.rs (feature "networkmanager")
pub fn subscription() -> Subscription<Option<NetworkInfo>>;
// common.rs
Message::Network(Option<NetworkInfo>)   // substitui NetworkIcon; continua preenchendo network_icon_opt
Message::Batteries(Vec<BatteryDevice>)
pub fn main_surface_of(&self, sub: SurfaceId) -> Option<SurfaceId>;   // subsurface_outputs → surface_ids
pub fn output_size(&self, surface: SurfaceId) -> Option<(u32, u32)>;  // subsurface_rects do output, arredondado
```
Regras: `classify` — `LinePower|Unknown|Monitor|Ups` → `None`; `Battery` → `Some(System)` só com
`power_supply && is_present`; periféricos (`Mouse, Keyboard, Headset, GamingInput, Phone, Tablet, Pen,
Touchpad, …`) → `Some(kind)` só com `percentage > 0`. Rede: nome = SSID (UTF-8 com perdas) no Wi-Fi, `Id` da
conexão ativa no cabo; Wi-Fi vence cabo como hoje. Atualização de periféricos: em `DeviceAdded`/`DeviceRemoved`
e a cada 5 min, enviando **só se mudou** (nada de redesenho periódico). Sem dado → lista vazia / `None`.

- [ ] **Passo 1 (testes que falham):** `classify_filters_line_power_and_empty` (LinePower→None, Mouse 0 %→None,
  Mouse 87 %→Some(Mouse), Battery sem power_supply→None, Battery ok→Some(System)); `glance_prefers_system`;
  `glance_falls_back_to_lowest_peripheral` (mouse 80, teclado 30 → teclado); `peripherals_skip_system_and_are_stable`;
  `network_icon_names_match_legacy` (Wired → `"network-wired-symbolic"`, Wireless 80 →
  `"network-wireless-signal-excellent-symbolic"`); `main_surface_of_maps_subsurface` (Common montado à mão com
  um output falso, se `WlOutput` puder ser construído em teste; senão testar a função pura que recebe os dois
  `HashMap`).
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte do greeter verde; `cargo build --release --no-default-features --features logind`
  compila (sysinfo não depende das features).
- [ ] **Passo 5:** Conferir diff (`git diff --stat`).

---

### Task 9: Primitivas visuais (`styles/parts.rs`)

**Onda 3 · depende de: 1 · trabalhador mínimo: `sonnet high`** (skill `frontend-design`)

**Files:**
- Modify: `greeter/src/styles/parts.rs`

**Interfaces — Consumes:** `Palette`, `FieldParts`, `PowerParts`, `ChoiceMenu`, `Look` (Task 1);
`login_ui::{avatar, power_button, password_input, FieldFx, Hovers}` (`login_ui.rs:378,749,467,183,177`).
**Não** chama funções da Task 6 (paralela): recebe cores prontas.
**Produces:**
```rust
pub fn card_spec(cfg: &StyleConfig) -> Option<(f32, f32)>;   // (opacidade, raio); None se card_opacity <= 0.01
pub fn card<'a, M: 'a>(content: Element<'a, M>, cfg: &StyleConfig, fill: Color, border: Color, alpha: f32) -> Element<'a, M>; // passa direto sem cartão
pub fn glass<'a, M: 'a>(content: Element<'a, M>, fill: Color, border: Color, shadow: Color, radius: f32, alpha: f32) -> widget::Container<'a, M>; // tinta + borda 1 px + sombra
pub struct FieldColors { pub fill: Color, pub text: Color, pub placeholder: Color, pub border: Color, pub accent: Option<Color> }
pub enum FieldShape { Win11, Pill { lock_icon: bool } }
pub fn field<'a, M: Clone + 'static>(parts: FieldParts<'a, M>, colors: FieldColors, shape: FieldShape, width: f32) -> Element<'a, M>;
pub enum ButtonShape { Circle, Squircle }
pub fn corner_button<'a, M: Clone + 'static>(icon: &'static str, on_press: M, fill: Color, icon_color: Color, shape: ButtonShape, size: f32, look: Look) -> Element<'a, M>;
pub fn power<'a, M: Clone + 'static>(p: PowerParts<M>, fill: Color, icon_color: Color, shape: Option<ButtonShape>) -> Element<'a, M>; // None = botão ⏻ atual do win11
pub fn choice<'a, M: Clone + 'static>(menu: ChoiceMenu<M>, fill: Color, text: Color, shape: Option<ButtonShape>, icon: &'static str, look: Look) -> Element<'a, M>; // None = texto ("Opções de entrada"); popover como `power_button`
pub fn text_line<'a, M: 'a>(s: Cow<'a, str>, size: f32, font: Font, color: Color, shadow: Color, alpha: f32, align: Horizontal) -> Element<'a, M>;
pub fn veil<'a, M: 'a>(content: Element<'a, M>, veil: Color /* alpha já calculado */, radius: f32) -> Element<'a, M>; // gradiente suave atrás; alpha 0 = passa direto
pub fn icon_row<'a, M: 'a>(icons: Vec<&'static str>, color: Color, size: u16, alpha: f32) -> Element<'a, M>; // vazio = nada
pub fn tile<'a, M: 'a>(value: String, label: String, icon: &'static str, fill: Color, border: Color, text: Color, alpha: f32) -> Element<'a, M>; // widget do iPadOS
pub fn home_bar<'a, M: 'a>(color: Color, width: f32, alpha: f32) -> Element<'a, M>;
```
O menu de `choice` reaproveita o padrão de `power_button`/`power_menu` (`login_ui.rs:725-781`: `popover` com
`Position::Point` e `on_close`), não o `dropdown_menu` do upstream.

- [ ] **Passo 1 (testes que falham):** `card_zero_opacity_means_no_card` — para cada estilo de
  `STYLE_NAMES`, `card_opacity = 0.0` → `card_spec == None`; `card_opacity = 0.3`, `card_radius = 4.0` →
  `Some((0.3, 4.0))` (sem mínimo por estilo: o antigo android forçava raio ≥ 24 e opacidade ≥ 0.42);
  `veil_zero_alpha_is_passthrough` (função pura auxiliar `veil_needed(c: Color) -> bool`: `a <= 0.005` → false).
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte do greeter verde.
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `parts.rs`).

---

### Task 10: Usuário e sessão padrão; fim do `todo!()` [RISCO]

**Onda 3 · depende de: 1, 2 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/selection.rs`
- Modify: `greeter/src/greeter.rs:1419-1452` (init: usuário/sessão), `:1580-1592` (`start_session`),
  `:1814-1850` (`Message::Username`: troca de sessão ao trocar de usuário)

**Interfaces — Consumes:** `StyleConfig::{default_user, session}` (Task 2), `fl!("session-not-found")`
(Task 1). Dados reais: `usernames: Vec<(String, String)>` ordenado por nome completo (`greeter.rs:1414-1419`),
`flags.user_datas[i].{name, uid}`, `greeter_config.last_user: Option<NonZeroU32>`,
`greeter_config.users[&uid].last_session: Option<String>`, `session_names` ordenado (`:1439-1440`), chaves de
`flags.sessions` = `Name` localizado do `.desktop` (`:296`). **Produces:**
```rust
pub fn pick_user(default_user: &str, last_user_uid: Option<u32>, users: &[(String, u32)]) -> Option<usize>;
    // default_user não vazio e presente (por nome) → ele; senão last_user presente → ele; senão 0; vazio → None
pub fn pick_session(configured: &str, last_session: Option<&str>, available: &[String]) -> Option<String>;
    // configured não vazio e presente → ele; senão last_session presente → ela; senão available[0]; vazio → None
```
`start_session`: se `selected_session` não está em `flags.sessions`, recalcula com
`pick_session(&cfg.session, None, &session_names)`; com `Some`, atualiza `selected_session` e segue; com
`None`, `self.common.error_opt = Some(fl!("session-not-found"))` e `Task::none()` — **nunca** `todo!()`/`panic!`.
`ConfigUpdateUser` (`:1860-1925`) continua gravando `last_user`/`last_session` como hoje.

- [ ] **Passo 1 (testes que falham):**
```rust
#[test] fn pick_user_default_wins_over_last() {
    let u = [("ana".into(), 1000), ("bia".into(), 1001)];
    assert_eq!(pick_user("bia", Some(1000), &u), Some(1));
}
#[test] fn pick_user_unknown_default_falls_to_last() { /* pick_user("zeca", Some(1001), &u) == Some(1) */ }
#[test] fn pick_user_empty_default_uses_last_then_first() { /* ("", Some(1001))→1; ("", Some(4242))→0; ("", None)→0 */ }
#[test] fn pick_user_no_users() { assert_eq!(pick_user("ana", None, &[]), None); }
#[test] fn pick_session_configured_wins() { /* ("COSMIC", Some("GNOME"), ["COSMIC","GNOME"]) → "COSMIC" */ }
#[test] fn pick_session_missing_configured_uses_last() { /* ("Plasma", Some("GNOME"), …) → "GNOME" */ }
#[test] fn pick_session_missing_last_uses_first() { /* ("", Some("Hyprland"), ["COSMIC","GNOME"]) → "COSMIC" */ }
#[test] fn pick_session_empty_list_is_none() { assert_eq!(pick_session("COSMIC", None, &[]), None); }
```
- [ ] **Passo 2:** rodar → falham.
- [ ] **Passo 3:** implementar `selection.rs` e usar em `init`, `Username` e `start_session`.
- [ ] **Passo 4:** suíte do greeter verde; `grep -n 'todo!\|unimplemented!' greeter/src/greeter.rs` → só o
  `view` de layer surface (`:1513`, pré-existente).
- [ ] **Passo 5 (controlador):** `scripts/dev/nested-greeter.sh` com `AENES_GREETER_CONFIG` apontando para um
  toml com `session = "Nao-Existe"`: senha `password` → o greeter falso aceita e **não** cai (log sem panic).
- [ ] **Passo 6:** Conferir diff (`git diff --stat`).

---

### Task 11: Prévia e lista de estilos do Painel

**Onda 3 · depende de: 3 · trabalhador mínimo: `sonnet medium`** (skill `frontend-design`)

**Files:**
- Modify: `src/control_panel/views.rs:4049-4315` (`login_live_preview`), `:4369-4401` (lista em `login_view`)

**Interfaces — Consumes:** `data::GREETER_STYLES` (Task 3); `data::GreeterConfig` (`data.rs:138-171`);
`Message::GreeterStyle(String)` (`mod.rs:314`).

Regras: chips gerados de `GREETER_STYLES` (grava o id novo); a prévia desenha a miniatura de cada estilo com as
mesmas proporções do protótipo (relógio, campo em pílula/retângulo, botões de canto, barrinha, coluna de
widgets de exemplo no iPadOS: "87 % Mouse", "Rede · Casa"); `card_opacity = 0` → sem cartão; `theme = light`
troca texto, campo e vidro; `blur_radius` > 0 suaviza o fundo da prévia (só tom, sem blur real); a legenda
mostra o nome do estilo (não o id). Sem acoplar ao crate do greeter.

- [ ] **Passo 1 (teste que falha):** `views.rs` ainda não tem `mod tests`; criar `#[cfg(test)] mod tests` no
  fim do arquivo com `style_label_uses_panel_names` — a função pura `fn style_label(id: &str) -> &'static str`
  (lê `GREETER_STYLES`) devolve `"iOS 26"` para `"ios"`, `"iPadOS 26"` para `"ipados"` e `"Windows 11"` para
  id desconhecido.
- [ ] **Passo 2:** suíte da raiz → falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte da raiz verde (linha de base + testes das Tasks 3 e 11).
- [ ] **Passo 5:** Conferir diff (`git diff --stat` → só `views.rs`).

---

### Tasks 12-16: os 5 estilos (um arquivo cada)

**Onda 4 · dependem de: 1, 2, 6, 7 (tipos), 8, 9 · trabalhador mínimo: `sonnet high`** (skill `frontend-design`)

Comum às cinco:
- **Files:** só `greeter/src/styles/<estilo>.rs` (≤ 400 linhas), substituindo o ramo legado; deixa de chamar
  `legacy_parts`.
- **Consumes:** `ScreenInput` (Task 1); `palette::{palette, style_font, min_alpha_for_contrast, FontRole}`
  (Task 6); `parts::*` (Task 9); `Analysis::region_luminance`, `Region` (Task 7); `SystemInfo::{glance_battery,
  peripherals}`, `DeviceKind::{icon_name,label_key}`, `NetworkInfo::icon_name` (Task 8).
- Cada estilo declara `pub const CLOCK_REGION: Region` e `pub const FIELD_REGION: Region` (normalizadas, onde
  o texto fica) e calcula: véu do relógio/data = `min_alpha_for_contrast(p.text, p.veil,
  analysis.region_luminance(CLOCK_REGION), 0.0)`; preenchimento do campo = mesmo cálculo com `floor =
  p.field_fill.a` sobre `FIELD_REGION`. Sem análise (`backdrop: None`) usa luminância 0.5.
- Respeita `show_date`, `show_avatar`, `show_username`, `clock_24h` (via `ClockData`), `clock_size`
  (`cfg.clock_scale()`), `card_opacity` (`parts::card`), `accent_color` (`p.accent`).
- Teste em cada arquivo: `fn regions_are_normalized()` — `0 <= x0 < x1 <= 1` e `0 <= y0 < y1 <= 1` para as
  duas regiões; e `fn view_builds_for_both_themes()` — monta `view` com `ScreenInput` de teste (cfg dark e
  light, `SystemInfo` com 1 mouse + rede, `backdrop: Some(&Analysis::uniform(0.9, [1.,1.,1.]))`, `field: None`)
  sem pânico (usar `Message = ()` só se compilar; senão um enum de teste com `Clone`).
- Passos: (1) testes que falham → (2) rodar → (3) implementar → (4) suíte do greeter verde → (5) print
  aninhado `scripts/dev/prints-estilos.sh locker` só do próprio estilo (dark e light) comparado ao protótipo
  pelo controlador → (6) Conferir diff (`git diff --stat` → só o arquivo do estilo).

| Task | Arquivo | Layout (spec, item 1) | Detalhes obrigatórios |
|---|---|---|---|
| 12 | `styles/win11.rs` | relógio Semibold centrado no topo, data abaixo; avatar redondo grande, nome; campo `FieldShape::Win11` (raio 4, linha de destaque, seta); `session` como `parts::choice(…, shape: None)` com `fl!("sign-in-options")` abaixo do campo; canto inferior direito: `parts::icon_row` (rede, bateria do sistema) + `parts::power(…, None)` | `show_seconds` vale; mantém `HoverTarget`/`FieldFx` atuais |
| 13 | `styles/android.rs` | linha "num relance" topo-esquerda (`clock.short_date` + `glance_battery`); barra de status mínima (hora, rede, bateria) topo; relógio gigante HH/MM em 2 linhas centrado; campo `Pill { lock_icon: true }` com cores tonais (`palette(cfg, backdrop.map(|a| a.average))`); `Squircle` energia à direita, sessão/teclado à esquerda | ignora `show_seconds`; sem ícone de digital |
| 14 | `styles/macos.rs` | data pequena acima do relógio enorme, arredondado (Nunito Bold) e translúcido; base: avatar pequeno (~40/62 do win11), nome, campo `Pill { lock_icon: false }` de vidro, dica `fl!("unlock-hint")`; ícones mínimos topo-direita (rede, bateria) + energia discreta | sessão (greeter) como `choice` texto discreto abaixo da dica |
| 15 | `styles/ios.rs` | barra de status (hora à esquerda, ícones à direita); data acima do relógio gigante ExtraBold; "degradê de vidro" aproximado (texto em branco translúcido + sombra; iced não pinta texto com gradiente — pode empilhar 2 cópias recortadas); campo de vidro em pílula; dois `Circle` de vidro nos cantos inferiores (energia à direita, sessão/teclado à esquerda); `home_bar` | antigo `editorial` |
| 16 | `styles/ipados.rs` | data e relógio grandes no canto superior esquerdo; coluna de `parts::tile` abaixo: um por `peripherals()` ("87 %" + `fl!(label_key)`/modelo) e um de rede (nome da conexão); campo de vidro centrado na base; `home_bar`; direita livre | widget sem dado não aparece; ordem dos widgets: periféricos (ordem estável) e rede por último |

---

### Task 17: Bloqueio — desfoque, análise, rotação e dados no `locker.rs` [RISCO]

**Onda 4 · depende de: 1, 2, 7, 8 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/locker.rs:515-560` (campos e `reload_style_and_wallpaper`), `:633-748` (`menu`),
  `:1130-1149` (init), `:1192-1200` (`view_window`), `:1202-1262` (`subscription`), `update_inner`
  (`Message::Common(Tick)` e nova `Message::Backdrop`) — **não tocar** `pam_thread`, `resolve_pam_service`,
  `Submit`, `SessionLockEvent`

**Interfaces — Consumes:** `backdrop::{BackdropSet, Tag, Prepared, handle_bytes, prepare_async, cache_dir,
surface_tag, pick_view_handle}` (Task 7); `StyleConfig::{rotation_slot, needs_second_tick}`,
`style_config::{rotation_due, wants_keyboard_menu}` (Task 2); `Common::{main_surface_of, output_size,
system_info}` (Task 8). **Produces:** `Message::Backdrop(SurfaceId, Result<backdrop::Prepared, String>)`.

Regras:
- `backdrops: BackdropSet<SurfaceId>` (chave = superfície principal). Para cada superfície com imagem
  (`custom_wallpaper` ou `common.surface_images[id]`) e tamanho conhecido: se `wants(id, tag)`, `mark_pending`
  e dispara `cosmic::task::future(prepare_async(bytes, tag, cache_dir(false)))`. Disparar no lock, ao chegar
  imagem/tamanho novos, e ao trocar papel de parede. Hash/decodificação só dentro do job.
- `view_window`: usa `prepared.blurred` quando `blur_radius > 0` e o tag bate; senão a imagem original —
  primeira pintura **nunca** espera o job. `dim_opacity` continua em `wallpaper_view`.
- `menu(sub)`: `backdrop: self.backdrops.get(main_surface_of(sub)?).map(|p| &p.analysis)`;
  `keyboard: Some(ChoiceMenu{…})` só com 2+ `active_layouts` (reusa `Dropdown::Keyboard`/`KeyboardLayout`).
- Rotação: em `Message::Common(Tick)`, se `rotation_slot(now)` mudou desde a última escolha, recarrega o papel
  de parede (sem mexer em `prompt_opt`, foco nem texto digitado) e dispara o job.
- `subscription`: o timer de 1 s passa a `self.style_config.needs_second_tick()`; nenhuma assinatura nova.
- `Err` do job → `tracing::warn!` uma vez por tag; tela segue com a original.

- [ ] **Passo 1:** sem teste unitário novo: toda a lógica pura (tag, troca de handle, rotação, menu de teclado,
  conjunto de backdrops) já é testada nas Tasks 2 e 7; esta task só liga. Rodar a suíte e anotar a contagem.
- [ ] **Passo 2:** implementar a ligação.
- [ ] **Passo 3:** `cargo build --release` sem warning novo.
- [ ] **Passo 4:** suíte do greeter verde (os testes `pam_service_*` e de demo do locker inalterados).
- [ ] **Passo 5 (controlador, aninhado):** `blur_radius = 24`: print logo na abertura (pode estar nítido) e após
  2 s (desfocado); `perf` do harness (`COSMIC_GREETER_PERF_LOG`) sem regressão no tempo do primeiro quadro;
  `top -b -n 3 -p <pid>` com a tela parada ≈ 0 % de CPU; com `rotate_interval` = 1 min, a imagem troca sem
  apagar texto já digitado no campo; 3 monitores reais só na Task 20.
- [ ] **Passo 6:** Conferir diff (`git diff --stat` → só `locker.rs`).

---

### Task 18: Login — desfoque, análise, rotação, sessão e dados no `greeter.rs` [RISCO]

**Onda 4 · depende de: 1, 2, 7, 8, 10 · trabalhador mínimo: `sonnet high`**

**Files:**
- Modify: `greeter/src/greeter.rs:570-572` (campos), `:622-753` (`menu`), `:1459-1463` (init do papel de
  parede), `:1517-1525` (`view_window`), `:1527-1552` (`subscription`), `update_inner` (`Tick`, `Backdrop`)

**Interfaces — Consumes:** as mesmas da Task 17 (todas de `backdrop.rs`/`style_config.rs`/`common.rs`, nada
de `locker.rs`) e `selection::pick_session` (Task 10).
**Produces:** `Message::Backdrop(SurfaceId, Result<backdrop::Prepared, String>)` no greeter.

Regras: iguais às da Task 17, com `cache_dir(true)` (boot: `/var/lib/aenes-greeter/cache`, criado pela Task 4;
ausente → `$XDG_CACHE_HOME` do `cosmic-greeter` ou sem cache); papel de parede inicial com
`pick_wallpaper_login_at(now)` em vez de `pick_wallpaper(0)`; `session: Some(ChoiceMenu{ label:
fl!("sign-in-options"), items: session_names marcados com selected_session, Message::Session })` reusando
`Dropdown::Session`/`DropdownToggle`; `keyboard` como no locker. `Username` limpa `surface_images` (`:1828`):
os backdrops dessas superfícies também são esquecidos (`forget`).

- [ ] **Passo 1 (teste que falha):** `login_initial_wallpaper_uses_time` — com dir temp e `rotate_on_lock`,
  a função pura usada no init (`fn initial_wallpaper(cfg: &StyleConfig, now: u64) -> Option<PathBuf>`, que
  delega a `pick_wallpaper_login_at`) devolve arquivos diferentes para `now = 0` e `now = 600`.
- [ ] **Passo 2:** rodar → falha.
- [ ] **Passo 3:** implementar.
- [ ] **Passo 4:** suíte do greeter verde.
- [ ] **Passo 5 (controlador, aninhado):** `nested-greeter.sh`: abrir o menu de sessão, trocar, entrar com
  `password` (greetd falso) → `StartSession` com a sessão escolhida no log; `blur_radius = 24` troca para
  desfocado sem atrasar a primeira pintura.
- [ ] **Passo 6:** Conferir diff (`git diff --stat` → só `greeter.rs`).

---

### Task 19: Remover o adaptador legado e código morto

**Onda 5 · depende de: 12-18 · trabalhador mínimo: `sonnet medium`**

**Files:**
- Modify: `greeter/src/styles/mod.rs` (remover `legacy_parts`)
- Modify: `greeter/src/login_ui.rs` (remover `maybe_glass_card`, `assemble_login_column`, `LoginBlock` e o que
  ficou sem uso; `menu_upstream` de locker/greeter fica como está)

- [ ] **Passo 1:** `cargo build --release` no greeter e listar os warnings `dead_code` de `login_ui.rs`/`styles`.
- [ ] **Passo 2:** remover só o que nenhum caminho usa (grep do símbolo em `src/` antes de cada remoção);
  remover os `#[allow(dead_code)]` provisórios da Task 1.
- [ ] **Passo 3:** build sem warning novo em relação à linha de base; suíte do greeter verde; `wc -l
  greeter/src/login_ui.rs` menor que 1499.
- [ ] **Passo 4:** Conferir diff (`git diff --stat`).

---

### Task 20: Verificação visual (prints) e verificação final

**Onda 6 · depende de: 19 · trabalhador mínimo: `sonnet medium`** (o controlador confere cada print)

**Files:** nenhum de código; saídas em `greeter/target/prints/` e `greeter/target/dev/`.

- [ ] **Passo 1:** suítes completas: greeter (linha de base da Task 0 + todos os testes novos, 0 falhas) e raiz
  (1039 + novos, 0 falhas); contagens no ledger.
- [ ] **Passo 2:** `cargo build --release` do greeter (binário e `--example server`) e da raiz.
- [ ] **Passo 3:** `greeter/scripts/dev/prints-estilos.sh locker` → 10 arquivos
  `locker-{win11,android,macos,ios,ipados}-{dark,light}.png`; `prints-estilos.sh greeter` só para `win11-dark`
  e `android-dark` (menu de sessão visível).
- [ ] **Passo 4:** repetir 2 combinações com `blur_radius = 24` e com `card_opacity = 0.0` vs `0.5`
  (`locker-ios-dark-blur.png`, `locker-win11-dark-card.png`).
- [ ] **Passo 5:** `scripts/dev/nested-panel.sh <estilo> dark` para os 5 estilos e `nested-panel.sh ipados
  light` → `painel-<estilo>-<tema>.png`.
- [ ] **Passo 6 (controlador):** conferir cada print contra o protótipo e a spec: estilo reconhecível, texto
  legível (sem texto claro sobre fundo claro), sem cartão quando `card_opacity = 0`, widgets só com dado,
  nenhum logotipo/ícone de digital. Lista de achados vai ao Diretor; **o usuário aprova os prints**.
- [ ] **Passo 7:** `git status --short` e `git diff --stat` finais por repositório no relatório; nada commitado.
- [ ] **Passo 8 (usuário, fora do agente):** instalar (`./install.sh`), bloquear (Win+L) e desbloquear com a
  senha correta; no máximo 1 senha errada por rodada (faillock `deny=3`).

---

## Self-review (feito)

- **Cobertura da spec:** obj. 1 → Tasks 12-16 (+1 despacho); 2 → Task 9 `card_spec` + estilos; 3 → Tasks 7,
  17, 18, 4 (script para de desfocar); 4 → Task 6 + regiões dos estilos; 5 → Tasks 2, 17, 18; 6 → Task 8 +
  13/16; 7 → Tasks 2, 10, 4; 8 → Tasks 3, 11; 9 → Task 6 `style_font` + Task 4 `install.sh`; teste quebrado do
  `offset.rs` → Task 0; scripts aninhados → Task 5; verificação → Tasks 20 e testes de cada task.
- **Divergências spec × código resolvidas aqui:** (a) a spec cita só os scripts, mas `nested-greeter.sh` precisa
  de `examples/server.rs`, ausente neste repo → Task 5 traz o exemplo; (b) não havia forma de trocar o
  `greeter.toml` do harness sem mexer no do usuário → override `AENES_GREETER_CONFIG` só quando
  `COSMIC_GREETER_NESTED=1` (Task 2); (c) o estilo atual não tem menu de sessão nem ícones de rede/bateria
  (`greeter.rs:622-753` não passa sessão; os ícones só existem em `menu_upstream`) → `ChoiceMenu` e
  `SystemInfo` no contrato (Task 1); (d) o script tem defaults diferentes do parser (`font_weight=light`,
  `clock_size=large`, `card_opacity=0.58`, `card_radius=18`) e escurece a cópia (dim em dobro) → Task 4;
  (e) `session` default é `"COSMIC"` no Painel mas o greeter trata vazio como "última sessão" → o greeter usa
  `""` por padrão e o Painel continua gravando o que o usuário escolher; (f) a correção PAM está só no working
  tree → pré-condição do worktree.
- **Consistência de tipos:** `ScreenInput`, `ChoiceMenu`, `Palette`, `Analysis`, `Region`, `Tag`, `Prepared`,
  `SystemInfo` definidos uma vez (Task 1 ou 7) e usados com o mesmo nome nas demais.
