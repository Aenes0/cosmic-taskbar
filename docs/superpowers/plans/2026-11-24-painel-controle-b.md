# Painel de Controle "B · Central de comando" — Plano de implementação

> **Para agentes:** SUB-SKILL OBRIGATÓRIA: superpowers:subagent-driven-development (conduzida pelo controlador da
> frente, conforme `~/.claude/docs/fabrica/protocolo-controlador.md`). Passos com `- [ ]`. **Não commite.**

**Objetivo:** reconstruir a interface do Painel de Controle (`cosmic-taskbar control-panel`) na direção B
(Visão geral + trilho de categorias + abas + busca Ctrl K + linha de ajuste única + barra "Aplicar" + avisos
tipados), preservando todas as configurações salvas e o comportamento.

**Arquitetura:** primeiro a lógica nova em módulos puros e testados (navegação, índice de busca, avisos, recentes,
pendência de aplicar, saúde de serviços, cálculo de métricas). Depois `views.rs` é quebrado em `views/` sem mudar
nada na tela. Por fim a moldura nova, o vocabulário de controles e cada seção são refeitos sobre essa base.

**Stack:** Rust, libcosmic (git, rev do `Cargo.lock`), iced. Crate único `cosmic-taskbar`.

**Spec:** `docs/superpowers/specs/2026-11-24-painel-controle-b-design.md` (ler inteira). Mockups:
https://claude.ai/artifact/7jbuYGLPDnbZieAQny7nuZ (artboards `Direcao-B`, `B-Energia`, `B-Busca`, `Linha`).

## Restrições globais

- **Pré-condição:** só começa depois que a entrega do greeter terminar (ela mexe em `views.rs`/`data.rs` na `main`).
  Branch e worktree são definidos com o usuário nessa hora (G0); sugestão `feature/painel-controle-b` em
  `~/Projetos/cosmic-taskbar-painel`.
- **Não tocar:** `greeter/`, `extras/greeter/`, `install.sh`, a lista de estilos/parsers de `GreeterConfig` em
  `data.rs`, o formato de `greeter.toml`. O bloco da Tela de Login (`fn style_label` … fim de `fn login_view`,
  incluindo `login_live_preview` e `preview_*`) só é **movido** para `views/login.rs`, mudando apenas linhas `use`.
- Helpers usados pelo login (`card_box`, `small_chip`, `toggle_row`, `action_btn`, `kv_row`, `status_badge`,
  `dimmed`, `icon`) ficam **idênticos** em `views/legacy.rs`. As seções novas não os usam.
- Nenhuma mudança de formato em: `power-profiles.conf`, `screenshot.conf`, config do OSD, configs do
  central/monitor/notificações, COSMIC Idle.
- Nada aplica no hardware nem reinicia serviço sem clique. Sem dependência nova.
- Espaçamentos só de `cosmic::theme::spacing()` (`space_xxxs` 4, `xxs` 8, `xs` 12, `s` 16, `m` 24, `l` 32).
  Cores de estado só de `theme.cosmic().{accent,success,warning,destructive}_color()`.
- Build e testes pelo host:
  `flatpak-spawn --host sh -c 'cd <WT> && cargo build --release 2>&1 | tail -3'` e
  `flatpak-spawn --host sh -c 'cd <WT> && cargo test --release 2>&1 | grep -E "^test result|FAILED|^error"'`.
  Linha de base registrada no Task 0; cada task fecha com **linha de base + N testes novos, 0 falhas**.
- Textos de UI em pt-BR, exatamente como na spec e nos mockups.
- Licença de corrigir o plano: se o código do brief não compilar, contradisser as assinaturas reais ou falhar nos
  testes do próprio brief, corrija para satisfazer o teste e relate a divergência; nunca altere o teste para
  acomodar implementação defeituosa.

## Foco de revisão

1. **Busca com acento e maiúscula** ("Énergia", "TELA", "recórte") acha o mesmo que sem acento → Task 2.
2. **Plano editado ≠ ativo e troca externa de plano** (Central ou `SettingsChanged`) → pendência recalculada,
   sem barra fantasma → Task 4.
3. **Rajada de avisos** (várias `ActionDone` seguidas) → no máximo 3 na tela, erro não some sozinho → Task 3.
4. **Unidade ausente / não instalada** não acende alerta; backup nunca rodou não quebra → Task 5.
5. **Janela no mínimo (780 × 540)**: trilho + abas quebram linha, paleta cabe, nada cortado → Task 15 (print em
   780 px obrigatório).

---

## Mapa de arquivos

| Arquivo | Responsabilidade | Task |
|---|---|---|
| `src/control_panel/nav.rs` (novo) | `Category`, ordem do trilho, seção → categoria, novos `from_arg` | 1 |
| `src/control_panel/search_index.rs` (novo) | Índice declarado + `search()` com normalização e ranking | 2 |
| `src/control_panel/feedback.rs` (novo) | Fila de avisos tipados e histórico de recentes | 3 |
| `src/control_panel/pending.rs` (novo) | Pendência de aplicar (Energia por plano, OSD) | 4 |
| `src/control_panel/health.rs` (novo) | `systemctl show` → saúde de unidade; backup atrasado | 5 |
| `src/shared/rates.rs` (novo) + `src/central/monitor/mod.rs` | Cálculo puro de CPU% e watts RAPL, usado pelo Monitor e pelo painel | 6 |
| `src/control_panel/metrics.rs` (novo) | Métricas da Visão geral a partir de duas leituras | 6 |
| `src/control_panel/views.rs` → raiz de `views/` | Quebra em módulos sem mudança visual | 7 |
| `src/control_panel/views/row.rs` (novo) | Vocabulário de controles e tons | 8 |
| `src/control_panel/views/shell.rs` (novo) + `mod.rs` | Moldura, mensagens novas, teclado, avisos, barra Aplicar | 9 |
| `views/home.rs`, `views/palette.rs` | Visão geral, paleta | 10, 11 |
| `views/sections/*.rs` | Seções refeitas | 12–14 |

---

### Task 0: Linha de base (Diretor)

- [ ] Confirmar com o usuário que o greeter terminou; G0 (branch/worktree); `git worktree add`.
- [ ] Rodar build e testes no worktree; registrar contagem em `progress.md`. Esperado (relatado pela entrega do
  greeter, conferir): ~1039 passed / 0 failed / 5 ignored.
- [ ] Recapturar `target/prints/antes/` se o greeter tiver mudado a aba Login (script em
  `target/prints/antes/.prints.sh`, janela do Claude fora do DP-3).

### Task 1: Navegação por categorias — Onda 1 · `sonnet` `medium`

**Files:** Create `src/control_panel/nav.rs`; Modify `src/control_panel/mod.rs` (enum `Section` ~49–160: novas
variantes, `ALL`, `label`, `icon`, `keywords`, `from_arg`; padrão de `init` ~535).

**Interfaces — Produces:**
- `Section::Home` e `Section::Capture` (novas). `Section::ALL` passa a ter 16, `Home` primeiro.
- `pub enum Category { Home, System, Appearance, Tools, Protection, Maintenance }` com
  `pub const ALL: [Category; 6]`, `fn label(self) -> &'static str` ("Início", "Sistema", "Aparência",
  "Ferramentas", "Proteção", "Manutenção"), `fn icon(self) -> &'static str`,
  `fn sections(self) -> &'static [Section]`.
- `pub fn category_of(s: Section) -> Category`.
- Ordem das seções (spec §5): System = [Power, Displays, Network, Camera]; Appearance = [Bar, Central, Monitor,
  Notifications, DevicesOsd, Login]; Tools = [Capture]; Protection = [Security, Backup];
  Maintenance = [Optional, System]; Home = [Home].
- Rótulos novos: Displays "Monitores & Janelas", Network "Rede & VPN", Capture "Captura de tela", Home "Visão geral".
- `Section::GROUPS` é removido no Task 9 (a lateral antiga ainda o usa até lá).

- [ ] **Teste (falha):** em `nav.rs`:
```rust
#[test] fn every_section_has_exactly_one_category() {
    for s in Section::ALL { assert_eq!(Category::ALL.iter().filter(|c| c.sections().contains(&s)).count(), 1, "{s:?}"); }
}
#[test] fn from_arg_keeps_old_values_and_accepts_new() {
    for (a, s) in [("power",Section::Power),("energia",Section::Power),("tray",Section::Bar),("rede",Section::Network),
                   ("monitores",Section::Displays),("greeter",Section::Login),("sistema",Section::System),
                   ("capture",Section::Capture),("captura",Section::Capture),("print",Section::Capture),
                   ("screenshot",Section::Capture),("home",Section::Home),("inicio",Section::Home),(" POWER ",Section::Power)] {
        assert_eq!(Section::from_arg(a), Some(s), "{a}");
    }
    assert_eq!(Section::from_arg("xyz"), None);
}
#[test] fn capture_and_osd_categories() {
    assert_eq!(category_of(Section::Capture), Category::Tools);
    assert_eq!(category_of(Section::DevicesOsd), Category::Appearance);
    assert_eq!(Category::ALL[0], Category::Home);
}
```
- [ ] Rodar `cargo test --release control_panel::nav` → FAIL (não compila).
- [ ] Implementar. `init`: sem `--section` abre em `Section::Home`. Até o Task 9, `Home` e `Capture` renderizam com
  um placeholder `text("…")` na `view` antiga (o `match` precisa ser exaustivo); a seção `Capture` ainda não tem
  conteúdo — o recorte continua em `displays_view` até o Task 13.
- [ ] Testes passam; suíte inteira verde (linha de base + 3).

### Task 2: Índice de busca — Onda 1 · `sonnet` `high`

**Files:** Create `src/control_panel/search_index.rs`. (Não tocar `mod.rs`; o registro `mod search_index;` é feito
pelo controlador ao integrar a onda, numa linha.)

**Interfaces — Produces:**
```rust
pub enum EntryKind { Setting, Action, Native }
pub struct Entry { pub kind: EntryKind, pub section: Section, pub group: &'static str,
                   pub label: &'static str, pub keywords: &'static str, pub advanced: bool,
                   pub native_page: Option<&'static str> /* argumento de `cosmic-settings` */ }
pub static INDEX: &[Entry];
pub fn normalize(s: &str) -> String;           // minúsculas, sem acento (á→a, ç→c, ê→e, õ→o…)
pub struct Results<'a> { pub settings: Vec<&'a Entry>, pub actions: Vec<&'a Entry>, pub native: Vec<&'a Entry> }
pub fn search(query: &str) -> Results<'static>; // máx 5 / 3 / 3
```
- `INDEX` contém **todos os rótulos de linha** das seções conforme o mockup (artboard `Main`, função `data()`),
  todos os botões de ação do cabeçalho das páginas e estas páginas nativas: ("Tela e bloqueio", "power"),
  ("Arranjo dos monitores", "displays"), ("Atalhos de teclado", "keyboard"), ("Bluetooth", "bluetooth"),
  ("Papel de parede", "wallpaper"), ("Som", "sound"), ("Data e hora", "time"), ("Usuários", "users").
  Conferir os nomes de página aceitos por `cosmic-settings --help` no host e ajustar.
- Ranking: rótulo começa com a busca > palavra do rótulo começa > contém no rótulo > contém em grupo/keywords.
  Empate: ordem do `INDEX`. Busca vazia → resultados vazios.

- [ ] **Teste (falha):**
```rust
#[test] fn accents_and_case_are_ignored() {
    assert_eq!(normalize("Énergia Ç TELA"), "energia c tela");
    let a = search("recórte"); let b = search("RECORTE");
    assert!(!a.settings.is_empty() || !a.actions.is_empty());
    assert_eq!(a.settings.iter().map(|e| e.label).collect::<Vec<_>>(), b.settings.iter().map(|e| e.label).collect::<Vec<_>>());
}
#[test] fn print_finds_capture() {
    assert!(search("print").settings.iter().all(|e| e.section == Section::Capture));
    assert!(!search("print").settings.is_empty());
}
#[test] fn limits_and_prefix_first() {
    let r = search("tela");
    assert!(r.settings.len() <= 5 && r.actions.len() <= 3 && r.native.len() <= 3);
    assert!(normalize(r.settings[0].label).starts_with("tela") || normalize(r.settings[0].label).split(' ').any(|w| w.starts_with("tela")));
    assert!(r.native.iter().any(|e| e.native_page == Some("power")));
}
#[test] fn empty_query_returns_nothing() { let r = search("  "); assert!(r.settings.is_empty() && r.actions.is_empty() && r.native.is_empty()); }
#[test] fn every_section_except_home_is_indexed() {
    for s in Section::ALL { if s != Section::Home { assert!(INDEX.iter().any(|e| e.section == s), "{s:?}"); } }
}
```
- [ ] FAIL → implementar → PASS; suíte verde.

### Task 3: Avisos tipados e recentes — Onda 1 · `sonnet` `medium`

**Files:** Create `src/control_panel/feedback.rs`.

**Interfaces — Produces:**
```rust
pub enum ToastKind { Success, Error, Info, Progress }
pub struct Toast<M> { pub id: u64, pub kind: ToastKind, pub text: String, pub undo: Option<M>, pub born: Instant }
pub struct Toasts<M> { /* fila */ }
impl<M: Clone> Toasts<M> {
    pub fn push(&mut self, kind: ToastKind, text: impl Into<String>, undo: Option<M>, now: Instant) -> u64;
    pub fn dismiss(&mut self, id: u64);
    pub fn expire(&mut self, now: Instant);          // Success/Info somem após TOAST_TTL; Error/Progress não
    pub fn visible(&self) -> &[Toast<M>];            // mais recente por último, máx MAX_VISIBLE
}
pub const TOAST_TTL: Duration = Duration::from_secs(5);
pub const MAX_VISIBLE: usize = 3;
pub fn kind_from_legacy(text: &str) -> ToastKind;   // "✗"/"Falha"/"Erro" → Error; "✓" → Success; senão Info
pub struct RecentChange<M> { pub label: String, pub section: Section, pub at: Instant, pub undo: Option<M> }
pub struct Recent<M> { /* máx 3 */ }
impl<M: Clone> Recent<M> { pub fn record(&mut self, c: RecentChange<M>); pub fn items(&self) -> &[RecentChange<M>]; pub fn take_undo(&mut self, idx: usize) -> Option<M>; }
```
- [ ] **Teste (falha):**
```rust
#[test] fn burst_keeps_three_and_errors_stay() {
    let t0 = Instant::now(); let mut q = Toasts::<u8>::default();
    q.push(ToastKind::Error, "Falha x", None, t0);
    for i in 0..5 { q.push(ToastKind::Success, format!("ok {i}"), None, t0); }
    assert_eq!(q.visible().len(), MAX_VISIBLE);
    assert!(q.visible().iter().any(|t| matches!(t.kind, ToastKind::Error)));
    q.expire(t0 + TOAST_TTL + Duration::from_millis(1));
    assert_eq!(q.visible().len(), 1);
}
#[test] fn legacy_prefixes() {
    assert!(matches!(kind_from_legacy("✗ deu ruim"), ToastKind::Error));
    assert!(matches!(kind_from_legacy("Falha ao salvar perfis"), ToastKind::Error));
    assert!(matches!(kind_from_legacy("✓ pronto"), ToastKind::Success));
    assert!(matches!(kind_from_legacy("Aplicando plano"), ToastKind::Info));
}
#[test] fn recent_keeps_last_three_and_undo_once() {
    let mut r = Recent::<u8>::default(); let now = Instant::now();
    for i in 0..4u8 { r.record(RecentChange { label: format!("c{i}"), section: Section::Power, at: now, undo: Some(i) }); }
    assert_eq!(r.items().len(), 3); assert_eq!(r.items()[0].label, "c3");
    assert_eq!(r.take_undo(0), Some(3)); assert_eq!(r.take_undo(0), None);
}
```
- [ ] FAIL → implementar → PASS; suíte verde.

### Task 4: Pendência de aplicar — Onda 1 · `sonnet` `high` (fase de risco: energia)

**Files:** Create `src/control_panel/pending.rs`. Ler `power_config.rs` (`PowerProfilesConfig`, `ProfileTuning`
~119–133, `PowerProfileKind`).

**Interfaces — Produces:**
```rust
pub struct Applied { /* último tuning aplicado por este painel, por plano; e hash do OSD no último reinício */ }
impl Applied {
    pub fn mark_power(&mut self, kind: PowerProfileKind, tuning: &ProfileTuning);
    pub fn forget_power(&mut self, kind: PowerProfileKind);   // ao trocar o plano ativo
    pub fn mark_osd(&mut self, cfg: &osd_config::Config);
}
pub enum ApplyAction { ApplyHardware, ActivateAndApply(PowerProfileKind) }
pub fn power_pending(cfg: &PowerProfilesConfig, applied: &Applied, editing: PowerProfileKind, active: Option<PowerProfileKind>) -> Option<ApplyAction>;
pub fn osd_pending(cfg: &osd_config::Config, applied: &Applied) -> bool;
pub fn hardware_fields_differ(a: &ProfileTuning, b: &ProfileTuning) -> bool; // ignora os 4 tempos de tela (vão ao Idle na hora)
```
- Regras (spec §6): sem marca para o plano → pendente só se `editing != active` (ativar) ou se o painel mudou algo
  nesta sessão; para isso o `ControlPanel` chama `mark_power` no `init` com o tuning carregado do plano ativo
  (considera-se aplicado o que estava no disco ao abrir). `taxa de atualização` conta como campo de hardware.
- [ ] **Teste (falha):**
```rust
#[test] fn active_plan_pending_only_after_hardware_change() {
    let mut cfg = PowerProfilesConfig::defaults_for(false, false); let mut ap = Applied::default();
    let k = PowerProfileKind::Balanced; ap.mark_power(k, cfg.get(k));
    assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
    cfg.get_mut(k).screen_off_mins = 99;            // tempo de tela: não é hardware
    assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
    cfg.get_mut(k).cpu_boost = !cfg.get(k).cpu_boost;
    assert!(matches!(power_pending(&cfg, &ap, k, Some(k)), Some(ApplyAction::ApplyHardware)));
}
#[test] fn editing_inactive_plan_offers_activate() {
    let cfg = PowerProfilesConfig::defaults_for(false, false); let ap = Applied::default();
    assert!(matches!(power_pending(&cfg, &ap, PowerProfileKind::Performance, Some(PowerProfileKind::PowerSaver)),
        Some(ApplyAction::ActivateAndApply(PowerProfileKind::Performance))));
}
#[test] fn external_switch_clears_pending() {
    let mut cfg = PowerProfilesConfig::defaults_for(false, false); let mut ap = Applied::default();
    let k = PowerProfileKind::Balanced; ap.mark_power(k, cfg.get(k)); cfg.get_mut(k).cpu_boost = !cfg.get(k).cpu_boost;
    ap.forget_power(k); ap.mark_power(k, cfg.get(k));   // o que o update faz ao receber troca externa
    assert!(power_pending(&cfg, &ap, k, Some(k)).is_none());
}
#[test] fn osd_pending_after_change() {
    let mut c = osd_config::Config::default(); let mut ap = Applied::default(); ap.mark_osd(&c);
    assert!(!osd_pending(&c, &ap)); c.pill_ms += 100; assert!(osd_pending(&c, &ap));
}
```
  Os nomes de campo (`screen_off_mins`, `cpu_boost`, `pill_ms`) são ilustrativos: usar os reais de
  `ProfileTuning` e `osd_config::Config` sem mudar a intenção de cada teste.
- [ ] FAIL → implementar → PASS; suíte verde.

### Task 5: Saúde de serviços e backup — Onda 1 · `sonnet` `medium`

**Files:** Create `src/control_panel/health.rs`.

**Interfaces — Produces:**
```rust
pub struct UnitHealth { pub loaded: bool, pub active: bool }
pub fn parse_show(text: &str) -> UnitHealth;                   // saída de `systemctl show -p LoadState,ActiveState`
pub fn is_stopped(h: &UnitHealth) -> bool;                     // loaded && !active
pub async fn unit_health(unit: &str, system: bool) -> UnitHealth;
pub struct TimerTimes { pub last: Option<SystemTime>, pub next: Option<SystemTime> }
pub fn parse_timer(text: &str) -> TimerTimes;                  // `-p LastTriggerUSec,NextElapseUSecRealtime --value`? usar `--timestamp=unix`
pub async fn backup_timer() -> TimerTimes;                     // unidade de usuário backup-restic.timer
pub const BACKUP_LATE: Duration = Duration::from_secs(26 * 3600);
pub fn backup_late(t: &TimerTimes, now: SystemTime) -> bool;   // last None → false (nunca rodou não é atraso)
```
- [ ] **Teste (falha):**
```rust
#[test] fn parse_unit_states() {
    assert!(is_stopped(&parse_show("LoadState=loaded\nActiveState=inactive\n")));
    assert!(!is_stopped(&parse_show("LoadState=not-found\nActiveState=inactive\n")));
    assert!(!is_stopped(&parse_show("LoadState=loaded\nActiveState=active\n")));
    assert!(!is_stopped(&parse_show("")));
}
#[test] fn backup_lateness() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000);
    let t = parse_timer("LastTriggerUSec=@1900000\nNextElapseUSecRealtime=@2040000\n");
    assert!(backup_late(&t, now));   // 100 000 s > 26 h
    assert!(!backup_late(&TimerTimes { last: None, next: None }, now));
    assert!(!backup_late(&parse_timer("LastTriggerUSec=n/a\n"), now));
}
```
- [ ] FAIL → implementar → PASS; suíte verde.

### Task 6: Cálculo de métricas compartilhado — Onda 1 · `sonnet` `high` (fase de risco: toca o Monitor)

**Files:** Create `src/shared/rates.rs`, `src/control_panel/metrics.rs`; Modify `src/shared/mod.rs` (registrar),
`src/central/monitor/mod.rs` (`take()` ~519–544 passa a chamar as funções puras; `mod sample;`/`mod gpu_share;`
viram `pub(crate) mod`).

**Interfaces — Produces:**
```rust
// shared/rates.rs
pub fn busy_pct(prev: (u64, u64), now: (u64, u64)) -> f32;                       // (busy,total); total 0 → 0
pub fn rapl_watts(prev: &[(u64, u64)], now: &[(u64, u64)], dt_secs: f32) -> Option<f32>; // (µJ, range) com volta
// control_panel/metrics.rs
pub struct HomeMetrics { pub cpu_pct: f32, pub cpu_temp: Option<f32>, pub cpu_ghz: Option<f32>,
    pub gpu_pct: Option<f32>, pub gpu_temp: Option<f32>, pub gpu_watts: Option<f32>,
    pub mem_pct: f32, pub total_watts: Option<f32>, pub cost_per_hour: Option<f32> }
pub fn compute(prev: &Reading, now: &Reading, gpu: Option<&Gpu>, rest_watts: f32, tariff: f32) -> HomeMetrics;
pub const POLL_FOCUSED: Duration = Duration::from_secs(2);
pub const POLL_UNFOCUSED: Duration = Duration::from_secs(10);
```
- A leitura usa `sample::reading(Depth { full: true, cpu: true, ram: true, gpu_sample: false, .. })` (não carrega
  NVML) e a GPU vem de `gpu_share::latest()`.
- [ ] **Teste (falha):**
```rust
#[test] fn busy_pct_basic_and_zero() { assert_eq!(busy_pct((100,1000),(150,1100)), 50.0); assert_eq!(busy_pct((5,5),(5,5)), 0.0); }
#[test] fn rapl_wraps() {
    let w = rapl_watts(&[(999_000_000, 1_000_000_000)], &[(1_000_000, 1_000_000_000)], 1.0).unwrap();
    assert!((w - 2.0).abs() < 1e-3);   // 1 000 000 + 1 000 000 µJ = 2 J em 1 s
    assert!(rapl_watts(&[], &[], 1.0).is_none());
}
```
  e em `metrics.rs` um teste de `compute` com duas `Reading` montadas à mão: CPU 50%, memória 25%,
  `total_watts = cpu + gpu + rest`, `cost_per_hour = total/1000*tariff`; GPU `None` → campos de GPU `None` e total
  só com CPU + resto.
- [ ] O teste existente do Monitor (`take`, ~mod.rs:1568) continua passando sem alteração.
- [ ] FAIL → implementar → PASS; suíte verde.

### Task 7: Quebrar `views.rs` sem mudar a tela — Onda 2 · `sonnet` `high`

**Files:** `src/control_panel/views.rs` vira raiz curta; Create `src/control_panel/views/{legacy,login,power,security,
backup,bar,monitor,central,notifications,osd,camera,network,displays,optional,system}.rs`.

- `legacy.rs`: os helpers atuais (linhas 35–194 e os demais usados por mais de uma seção) **sem alteração**.
- `login.rs`: bloco do login copiado byte a byte; só os `use` mudam. Conferir com
  `diff <(sed -n '/^fn style_label/,/^fn optional_view/p' views.rs.orig) views/login.rs` (só linhas `use` e o
  fim do bloco diferem).
- Uma seção por arquivo, cada `*_view` com a mesma assinatura `fn x_view(app: &ControlPanel) -> Element<'_, Message>`.
- [ ] Build e suíte verdes (mesma contagem do Task 6).
- [ ] Prints de 3 abas (Energia, Segurança, Login) idênticos aos de antes (comparar visualmente; o Login é o
  crítico).

### Task 8: Vocabulário de controles — Onda 3 · `sonnet` `high` · skill `frontend-design`

**Files:** Create `src/control_panel/views/row.rs`.

**Interfaces — Produces** (todas `-> Element<'a, Message>`; `desc: Option<&'a str>`):
```rust
pub enum Tone { Ok, Warn, Bad, Info, Neutral }
pub fn tone_colors(theme: &cosmic::Theme, t: Tone) -> (Color /*fundo 14%*/, Color /*texto*/);
pub fn group<'a>(title: &'a str, desc: Option<&'a str>, advanced: bool, rows: Vec<Element<'a, Message>>) -> Element<'a, Message>; // cartão, linhas com divisória
pub fn toggle<'a>(label: &'a str, desc: Option<&'a str>, on: bool, msg: impl Fn(bool) -> Message + 'a) -> Element<'a, Message>;
pub fn select<'a>(label: &'a str, desc: Option<&'a str>, options: &'a [String], selected: Option<usize>, on: impl Fn(usize) -> Message + Send + Sync + 'static) -> Element<'a, Message>; // dropdown
pub fn segmented<'a>(label: &'a str, desc: Option<&'a str>, options: &[&'a str], selected: usize, on: impl Fn(usize) -> Message + 'a) -> Element<'a, Message>; // botões num contêiner, sem Model
pub fn slider<'a>(label: &'a str, desc: Option<&'a str>, range: RangeInclusive<f32>, step: f32, value: f32, shown: String, min_max: (String, String), on: impl Fn(f32) -> Message + 'a, on_release: Option<Message>) -> Element<'a, Message>;
pub fn status<'a>(label: &'a str, desc: Option<&'a str>, pill: &'a str, tone: Tone, buttons: Vec<Element<'a, Message>>) -> Element<'a, Message>;
pub fn kv<'a>(label: &'a str, value: String) -> Element<'a, Message>;
pub fn input<'a>(label: &'a str, desc: Option<&'a str>, placeholder: &'a str, value: &'a str, on: impl Fn(String) -> Message + 'a, button: Option<(&'a str, Message)>) -> Element<'a, Message>;
pub fn buttons<'a>(label: &'a str, desc: Option<&'a str>, buttons: Vec<Element<'a, Message>>) -> Element<'a, Message>;
pub fn button<'a>(label: &'a str, msg: Message, style: BtnStyle) -> Element<'a, Message>; // BtnStyle { Primary, Normal, Danger }
pub fn pill<'a>(text: &'a str, tone: Tone) -> Element<'a, Message>;
pub fn note<'a>(text: &'a str) -> Element<'a, Message>;
pub fn list_item<'a>(name: &'a str, meta: String, pill: Option<(&'a str, Tone)>, button: Option<Element<'a, Message>>) -> Element<'a, Message>;
pub fn swatches<'a>(label: &'a str, desc: Option<&'a str>, items: &[(&'a str, Color)], selected: Option<usize>, on: impl Fn(usize) -> Message + 'a) -> Element<'a, Message>;
```
- Medidas do mockup `Linha`: linha com padding 14×18, rótulo 14 px médio, descrição 12,5 px com `dimmed`,
  controle à direita até 60% da largura; slider com selo de valor em destaque e mínimo/máximo abaixo.
- [ ] Teste puro: `tone_colors` com o tema escuro padrão devolve as cores de `success/warning/destructive/accent`
  do tema (não RGB fixo).
- [ ] Uma tela de demonstração temporária não entra no código; a conferência visual é feita no Task 9.

### Task 9: Moldura, mensagens e teclado — Onda 3 · `sonnet` `high` (fase de risco: `mod.rs` inteiro)

**Files:** Create `views/shell.rs`; Modify `mod.rs` (estado, `Message`, `update`, `subscription`, `header_*`),
`views.rs` (raiz: `view()` usa a moldura nova).

**Estado novo em `ControlPanel`:** `toasts: Toasts<Message>`, `recent: Recent<Message>`, `applied: Applied`,
`palette: Option<PaletteState { query: String, cursor: usize }>`, `metrics: Option<HomeMetrics>`,
`last_reading: Option<Reading>`, `focused: bool`, `advanced_open: bool` (substitui `power_show_advanced` e
`security_show_advanced`), `scroll_to: Option<&'static str>` (grupo alvo). Sai `notice` (as 45 atribuições viram
`self.toast(kind_from_legacy(&t), t)` ou um tipo explícito).

**`Message` novas:** `SelectCategory(Category)`, `OpenPalette`, `ClosePalette`, `PaletteQuery(String)`,
`PaletteMove(i32)`, `PaletteActivate { run: bool }`, `OpenNative(&'static str)`, `JumpTo(&'static str)`,
`ToggleAdvanced`, `ApplyPower`, `ApplyOsd`, `DismissToast(u64)`, `Undo(usize)`, `Tick`, `MetricsTick`,
`WindowFocus(bool)`. Saem `PowerSaveOnly`, `OsdSaveOnly`, `DismissNotice`, `TogglePowerAdvanced`,
`ToggleSecurityAdvanced`; `PowerSaveAndApply` vira `ApplyPower` (mesmo corpo, sem o `save()`; chama
`applied.mark_power`); `OsdSaveAndRestart` vira `ApplyOsd` (chama `applied.mark_osd`); `SetPowerProfile` e a troca
vinda de `Update::PowerProfiles` chamam `forget_power` + `mark_power` do novo ativo.

**Teclado** (`event::listen_with`, só sem modal do sistema): Ctrl+K e "/" (sem campo focado) → `OpenPalette`;
Esc → `ClosePalette`; ↑/↓ → `PaletteMove`; Enter / Ctrl+Enter → `PaletteActivate`. Foco da janela →
`WindowFocus`. `Tick` a cada 1 s só enquanto houver aviso visível. `MetricsTick` com `POLL_FOCUSED` /
`POLL_UNFOCUSED` só quando `section == Home`.

**Moldura** (`shell.rs`, medidas do mockup `Direcao-B`): cabeçalho (`header_start` título; `header_center` botão
de busca 480 px com "Ctrl K"; `header_end` "Tudo salvo" / "1 ajuste para aplicar"), trilho 92 px com ponto âmbar
por categoria com alerta e "Atualizar status" no pé, trilha "Início › Categoria", abas sublinhadas, cabeçalho da
página (ícone 44, título 22, subtítulo, selo, ações), "Ir para", botão "Mostrar/Ocultar ajustes avançados", barra
Aplicar fixa no rodapé, pilha de avisos no canto inferior direito. As seções ainda antigas renderizam dentro dela.

`busy` continua existindo, mas passa a desabilitar os botões de ação da página e a barra Aplicar (`on_press_maybe`)
e a mostrar um aviso `Progress`; some o texto "Executando ação..." da lateral.

- [ ] Testes puros novos em `mod.rs`: `fn apply_bar_label(pending: &ApplyAction, editing: PowerProfileKind) -> String`
  ("Aplicar no hardware" / "Ativar e aplicar Desempenho"); `fn save_status(pending_count: usize) -> String`
  ("Tudo salvo" / "1 ajuste para aplicar" / "2 ajustes para aplicar").
- [ ] Build e suíte verdes; prints da moldura com Energia e Login (Login intacto por dentro).

### Task 10: Visão geral — Onda 4 · `sonnet` `high`

**Files:** Create `views/home.rs`. Consome `HomeMetrics`, `health`, `Recent`, `pending`.
Conteúdo e ordem exatos da spec §7 e do artboard `Direcao-B`. Seletor de plano do cartão Energia →
`SetPowerProfile`. Alertas a partir de: `is_stopped` das unidades rastreadas, nota de proteção < recomendada,
`backup_late` ou destino desmontado, pendências. Teste puro: `fn alerts(app_state…) -> Vec<Alert>` com casos
"nada a resolver → vazio" e "unidade de sistema parada → alerta com ação 'Abrir'".

### Task 11: Paleta Ctrl K — Onda 4 · `sonnet` `high`

**Files:** Create `views/palette.rs`. Overlay centrado 640 px, campo com foco ao abrir, grupos Ajustes / Ações /
COSMIC Settings, primeiro item destacado, rodapé de atalhos (artboard `B-Busca`). `PaletteActivate` em ajuste →
seção + `advanced_open` se `entry.advanced` + `JumpTo(group)`; em nativo → `OpenNative(page)` roda
`cosmic-settings <page>` destacado. Teste puro de `fn cursor_target(results, cursor) -> Option<&Entry>` com
cursor passando do fim (volta ao começo) e resultados vazios (`None`).

### Tasks 12–14: Seções no vocabulário novo — Onda 4 · `sonnet` `high` · arquivos disjuntos

Cada uma refaz os arquivos `views/<seção>.rs` só com `row.rs`, seguindo spec §5, §9 e §10 e o artboard `Main`
(`data()` lista cada linha, rótulo e descrição). Mapeamento de chips → controles da spec §9. Nenhuma `Message`
nova além das do Task 9; se faltar uma, `STATUS: PERGUNTA`.
- **Task 12 (risco):** `power.rs` (seletor "Editando: [plano ▾]" sempre visível, grupos por plano com o nome do
  plano no título, Bateria e Periféricos antes do avançado, Kernel no avançado), `osd.rs`.
- **Task 13:** `displays.rs` (sem recorte), `capture.rs` (novo, conteúdo do recorte), `network.rs` (+ MAC via o
  `RunAction` pkexec que Segurança usa hoje), `camera.rs`.
- **Task 14:** `bar.rs`, `central.rs`, `monitor.rs`, `notifications.rs` (prévia primeiro, amostras + hex),
  `security.rs` (DNS segmentado, selo "Bem protegido · N/100", kill switch destrutivo, sem MAC), `backup.rs`
  (horários do timer), `optional.rs` (sem MAC; "Abrir Mission Center" uma vez), `system.rs`.
- Ações duplicadas viram uma só (spec §5): "Restaurar tiles" só no cabeçalho da Central; "Testar OSD" só no OSD
  (Notificações fica com "Enviar teste"); "Corrigir ícones PWA" só em Monitores; o título de cada página é o rótulo
  da aba (some o `title3` próprio).
- Cada task: build e suíte verdes + prints das suas seções em `target/prints/depois/`.

### Task 15: Limpeza e verificação final — Onda 5 · controlador

- [ ] Remover helpers de `legacy.rs` que só seções antigas usavam (o login continua compilando com os seus),
  `Section::GROUPS`, `page_banner`, `section_subtitle`.
- [ ] `grep -n "spacing([0-9]" src/control_panel/views/` sem resultados fora de `legacy.rs`/`login.rs`;
  `grep -n "Color::from_rgb" src/control_panel/views/` sem resultados fora de `legacy.rs`/`login.rs`/prévias.
- [ ] Build e suíte verdes (linha de base + todos os novos).
- [ ] Prints `depois` de todas as seções em 1280 × 820 e de Início, Energia e paleta em **780 × 540**.
- [ ] `git diff --stat` não mostra `greeter/`, `extras/greeter/`, `install.sh`; `diff` do bloco do login só em
  `use`.

## Ondas e paralelismo

| Onda | Tasks (arquivos disjuntos) | Depende de |
|---|---|---|
| 1 | 1, 2, 3, 4, 5, 6 (em paralelo; registro dos `mod` feito pelo controlador) | 0 |
| 2 | 7 | 1 |
| 3 | 8, depois 9 | 2–7 |
| 4 | 10, 11, 12, 13, 14 (em paralelo) | 9 |
| 5 | 15, revisão final (`revisor`), entrega com prints antes/depois | 10–14 |
