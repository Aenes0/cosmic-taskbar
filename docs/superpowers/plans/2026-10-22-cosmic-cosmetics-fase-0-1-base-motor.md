# cosmic-cosmetics — Fases 0 e 1 (base, motor, núcleo, holder) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Um workspace Rust `cosmic-cosmetics` com linha de base de consumo medida, motor de renderização leve (formas, texto, tema ao vivo, animação, ícones, superfície layer-shell, entrada com IME, texto editável, a11y), núcleo de módulos supervisionados com config e recarga a quente, processo `holder` que segura a zona da barra, units systemd e um daemon "hello" que prova tudo isso no compositor aninhado.

**Architecture:** Workspace Cargo com `engine` (sem estado de domínio), `core` (trait `Module`, supervisor, config) e três binários: `cosmic-cosmetics-holder`, `cosmic-cosmeticsd`, e (esqueleto vazio) o resto fica para fases futuras. `engine` nasce copiando e adaptando `notificacoes/src/render/*` e `timing.rs`; nada de libcosmic/iced no daemon. Eventos via `calloop` (modelo único do `engine` e do `core`).

**Tech Stack:** Rust edition 2024 (rust-version 1.88), cosmic-text 0.19, tiny-skia 0.11, smithay-client-toolkit 0.20 (+calloop), xkbcommon, accesskit, resvg, zbus 5 (D-Bus), inotify, systemd user units.

**Spec:** `docs/superpowers/specs/2026-10-22-cosmic-cosmetics-design.md` (§3 arquitetura, §3.1.1 modelo de processos, §3.2 motor, §6 config, §9 fases 0–1, §10 aceite).

## Global Constraints

- Rust edition 2024, `rust-version = "1.88"`. Nenhum perfil usa `panic = "abort"` no daemon: o supervisor precisa capturar panic de módulo (spec §3.1). O holder pode usar `abort` (não carrega módulos).
- Sem libcosmic/iced em `engine`, `core`, holder e daemon (spec §3.2, §8). Permitido apenas na janela de configurações (fase 9).
- `cosmic-protocols`/`cctk`: usar uma única `rev` para todo o workspace; ponto de partida `c0cff4db14c37ed954983158e4055aa94c7741d9` (a do `notificacoes`), validar contra o COSMIC instalado (spec §11.1).
- CPU em idle ≈ 0: nenhum polling; só eventos (Wayland, D-Bus, inotify) e um timer único no prazo mais próximo (spec §2.3).
- IME via text-input-v3 e AccessKit já na v1 (spec §3.2).
- Tema do COSMIC ao vivo: cores, raio, modo escuro/claro, alto contraste, fonte, espaçamento (spec §3.2).
- Daemon: nunca reiniciar por causa de módulo; holder: nunca carrega módulos (spec §3.1.1).
- Repositório novo em `~/Projetos/cosmic-cosmetics` **[escolha do redator; a spec deixa o local em aberto]**. Trabalho direto em `main` (decidido), **sem commits** (CLAUDE.md §3.1): os arquivos ficam no working tree.
- Ambiente: o shell do Claude é um Flatpak sandbox sem `cargo`/`git`; comandos de build/teste rodam via `flatpak-spawn --host sh -c '…'`. Daqui em diante `HOST` = `flatpak-spawn --host sh -c`.
- Cada arquivo de código < 800 linhas; funções < 50 linhas (regras globais).
- Todo texto de interface e mensagens de erro em PT-BR (i18n PT/EN fica para o módulo que expõe UI).

## Review Focus

Casos que a spec implica e que nenhuma tarefa óbvia cobriria; cada linha tem o teste na tarefa dona:

1. **Arquivo de tema do COSMIC lido no meio de uma escrita (truncado/vazio)** → o tema anterior é mantido, sem panic nem cor preta (Task 5).
2. **Config do módulo truncada ou com TOML inválido** → o módulo mantém a config anterior e reporta erro; o daemon não cai (Task 12).
3. **Módulo que paniquea ou trava dentro de `start`/`handle`** → só ele é reiniciado, com backoff, e os outros continuam (Task 11).
4. **Preedit de IME com cursor no meio de um grafema composto (emoji com ZWJ, acento combinante)** → o cursor nunca cai no meio de um cluster (Task 9).
5. **Reinício do daemon com monitor desconectado/novo (hotplug) durante o handover da zona exclusiva** → o holder mantém a zona para os outputs existentes e não vaza superfícies (Task 13).

---

### Task 1: Workspace e repositório novo

**Files:**
- Create: `~/Projetos/cosmic-cosmetics/Cargo.toml` (workspace), `rust-toolchain.toml` (1.88), `.gitignore`
- Create: `crates/engine/Cargo.toml`, `crates/engine/src/lib.rs`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`
- Create: `bin/holder/Cargo.toml`, `bin/holder/src/main.rs`
- Create: `bin/daemon/Cargo.toml`, `bin/daemon/src/main.rs`
- Create: `scripts/run-host.sh` (envolve `flatpak-spawn --host sh -c`)
- Test: `crates/engine/tests/smoke.rs`

**Interfaces:**
- Produces: workspace com membros `crates/engine` (lib `engine`), `crates/core` (lib `cosmic_core`, pacote `core` renomeado para não colidir com `core` do Rust), `bin/holder` (bin `cosmic-cosmetics-holder`), `bin/daemon` (bin `cosmic-cosmeticsd`). `[workspace.dependencies]` fixa as versões da tabela "Tech Stack".

- [ ] **Step 1: Escrever o teste de fumaça** em `crates/engine/tests/smoke.rs`: `fn engine_version_is_exposed()` assertando `engine::VERSION == env!("CARGO_PKG_VERSION")`.
- [ ] **Step 2: Rodar e ver falhar:** `HOST 'cd ~/Projetos/cosmic-cosmetics && cargo test -p engine'` → esperado: erro de compilação (`VERSION` não existe).
- [ ] **Step 3: Criar o workspace** com os quatro membros e `pub const VERSION: &str` em `engine`. `[profile.release]` com `opt-level=3, lto="thin", codegen-units=1, strip=true`; `panic` não definido (unwind) por causa do supervisor.
- [ ] **Step 4: Rodar:** `cargo test -p engine && cargo check --workspace` → esperado: `1 passed` e `Finished`.

---

### Task 2: Linha de base de consumo dos projetos atuais

Mede RSS/PSS, wakeups e CPU dos processos atuais (spec §2.1, §10.2) para o critério "menor que a soma".

**Files:**
- Create: `scripts/measure-baseline.sh`, `scripts/lib/proc_sample.py` (lê `/proc/<pid>/{smaps_rollup,stat,status}`)
- Create: `docs/baseline/PERIMETRO.md` (lista de processos "antes", conforme spec §10.2)
- Test: `scripts/tests/test_proc_sample.py` com fixture em `scripts/tests/fixtures/proc/`

**Interfaces:**
- Produces: `proc_sample.sample(pid: int, proc_root: str = "/proc") -> dict` com chaves `pid, comm, rss_kb, pss_kb, utime_ticks, stime_ticks, voluntary_ctxt, nonvoluntary_ctxt, threads`. `measure-baseline.sh <segundos>` imprime um JSON `{"processes":[…],"total_pss_kb":N,"interval_s":S}`.

- [ ] **Step 1: Escrever o teste** `test_sample_reads_fixture`: fixture com `smaps_rollup` (`Rss: 4500 kB`, `Pss: 3100 kB`), `status` (`Threads: 3`, `voluntary_ctxt_switches: 120`, `nonvoluntary_ctxt_switches: 4`) e `stat`; assertar `rss_kb == 4500`, `pss_kb == 3100`, `threads == 3`, `voluntary_ctxt == 120`. Segundo teste: pid inexistente levanta `ProcessLookupError`.
- [ ] **Step 2: Rodar:** `HOST 'cd ~/Projetos/cosmic-cosmetics && python3 -I -m pytest scripts/tests -q'` → esperado: FAIL (módulo não existe).
- [ ] **Step 3: Implementar `proc_sample.sample`** (só stdlib) e `measure-baseline.sh`, que localiza por `comm`/cmdline os processos do perímetro (`cosmic-panel`, applets, `notificacoes`, `central*`, `cosmic-workspaces`, `logi-osd`, `taskbar`, `start-menu`, `iriun`, etc.), amostra no início e no fim do intervalo e calcula a diferença de context switches (wakeups) e CPU.
- [ ] **Step 4: Rodar os testes** → `2 passed`.
- [ ] **Step 5: Medir de verdade:** `HOST 'cd ~/Projetos/cosmic-cosmetics && scripts/measure-baseline.sh 60 > docs/baseline/antes.json'` com a sessão normal do Alexandre aberta; conferir que `total_pss_kb > 0` e que cada projeto do perímetro apareceu (lista impressa no stderr). Registrar em `docs/baseline/PERIMETRO.md` quem entrou e quem faltou. O número-alvo da meta 1 fica "≤ `total_pss_kb`" até o Alexandre fixar outro (spec §12).

---

### Task 3: engine — cor e formas

Extrai de `notificacoes/src/render/{theme.rs (Rgba), shapes.rs}`.

**Files:**
- Create: `crates/engine/src/color.rs`, `crates/engine/src/shapes.rs`
- Modify: `crates/engine/src/lib.rs` (`pub mod color; pub mod shapes;`)
- Test: `crates/engine/tests/shapes.rs`

**Interfaces:**
- Produces: `color::Rgba(pub u8, pub u8, pub u8, pub u8)` (`Copy, PartialEq, Debug`), `color::parse_hex(&str) -> Option<Rgba>`; `shapes::{skia_color(Rgba)->tiny_skia::Color, rounded_rect_path(tiny_skia::Rect, f32)->Option<tiny_skia::Path>, fill_rounded_rect(&mut Pixmap, Rect, f32, Rgba), stroke_rounded_rect(&mut Pixmap, Rect, f32, f32, Rgba), fill_pill(&mut Pixmap, Rect, Rgba), draw_cross(&mut Pixmap, f32, f32, f32, f32, Rgba), draw_shadow(&mut Pixmap, Rect, radius: f32, sigma: f32, dy: f32, alpha: f32)}`.

- [ ] **Step 1: Testes:** `fill_rounded_rect_paints_center_not_corner` (pixmap 40×40, rect cheio, raio 12, cor `Rgba(255,0,0,255)`: pixel (20,20) é vermelho, pixel (0,0) tem alpha 0); `parse_hex_accepts_6_and_8_digits` (`"#E5534B"` → `Rgba(0xE5,0x53,0x4B,0xFF)`, `"#E5534B80"` → alpha 0x80, `"E5534B"` sem `#` → `Some`, `"zzz"` → `None`); `shadow_is_darker_below_than_above` (soma de alpha das 10 linhas abaixo do cartão > soma das 10 acima, com `dy=4`).
- [ ] **Step 2: Rodar** `cargo test -p engine --test shapes` → FAIL.
- [ ] **Step 3: Portar** as funções de `notificacoes/src/render/shapes.rs` (185 LOC) e `Rgba`/`parse_hex` de `theme.rs`, sem mudar comportamento; a sombra gaussiana por máscara f32 continua sem dependência extra.
- [ ] **Step 4: Rodar** → `3 passed`.

---

### Task 4: engine — texto

Extrai `notificacoes/src/render/text.rs` (cosmic-text com wrap, reticências, emoji colorido, RTL/CJK).

**Files:**
- Create: `crates/engine/src/text.rs`
- Test: `crates/engine/tests/text.rs`

**Interfaces:**
- Consumes: `color::Rgba`.
- Produces: `text::TextStyle { px: f32, weight: u16, line_height: f32 }` com consts `TITLE, APP, BODY, BUTTON` (mesmos valores de `notificacoes`: 13/600/1.3, 11/400/1.3, 12/400/1.35, 13/600/1.2); `text::Text::new(family: Option<&str>) -> Text`; `Text::layout(&mut self, s: &str, style: TextStyle, max_width: f32, max_lines: usize, scale: u32) -> Laid`; `Laid { width: f32, height: f32, lines: usize, … }`; `Text::draw(&mut self, pm: &mut Pixmap, laid: &Laid, x: f32, y: f32, scale: u32, color: Rgba)`; `Text::release_caches(&mut self)`; `text::interface_font_family(cosmic_dir: &Path) -> Option<String>`.

- [ ] **Step 1: Testes:** `layout_wraps_long_text_within_max_width` (texto de 200 caracteres, `max_width=120`, `max_lines=3` → `laid.lines == 3` e `laid.width <= 120.0`); `layout_empty_string_has_zero_height`; `layout_scale_doubles_height` (mesmo texto com `scale=2` tem altura ≈ 2× a de `scale=1`, tolerância 1 px); `missing_font_family_falls_back` (`Text::new(Some("FonteQueNaoExiste"))` não paniquea e `layout("abc", BODY, 100.0, 1, 1).width > 0.0`).
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Portar** `Text`/`Laid` e `interface_font_family` (310 + trecho de ~40 LOC), mantendo o limite de cache e `release_caches` (a RAM no idle importa para a meta 1).
- [ ] **Step 4: Rodar** → `4 passed`.

---

### Task 5: engine — tema do COSMIC ao vivo

`notificacoes` relê o tema só a cada aviso e sem inotify; aqui o tema é um fluxo de mudanças.

**Files:**
- Create: `crates/engine/src/theme.rs`, `crates/engine/src/watch.rs` (inotify sobre calloop)
- Test: `crates/engine/tests/theme.rs`

**Interfaces:**
- Consumes: `color::Rgba`, `color::parse_hex`.
- Produces:
  - `theme::Theme { dark: bool, high_contrast: bool, background: Rgba, text: Rgba, accent: Rgba, corner_radius: f32, spacing_unit: f32 }` e `theme::FALLBACK: Theme`;
  - `theme::load(cosmic_dir: &Path) -> Result<Theme, ThemeError>` (lê `com.system76.CosmicTheme.{Dark,Light}/vN/{background,accent}`, modo em `com.system76.CosmicTheme.Mode/vN/is_dark`, escolhe o maior `vN`);
  - `theme::ThemeWatcher::new(cosmic_dir: PathBuf, handle: calloop::LoopHandle<'_, D>, on_change: impl FnMut(&mut D, Theme) + 'static) -> Result<ThemeWatcher>`.

- [ ] **Step 1: Testes** com diretório temporário que imita a árvore do COSMIC:
  - `load_picks_highest_version_dir` (cria `v1` e `v3`, cores diferentes, espera a de `v3`);
  - `load_uses_dark_dir_when_mode_is_dark` e `…_light_…`;
  - `load_missing_files_returns_error_not_panic`;
  - `partial_write_keeps_previous_theme` (**Review Focus 1**): `ThemeWatcher` já entregou o tema A; um arquivo `background` é truncado para 0 bytes e depois reescrito válido (tema B); asserte que o callback nunca recebeu um tema com `background == Rgba(0,0,0,0)` entre A e B e que recebeu B ao final;
  - `change_is_debounced` (5 escritas em < 50 ms geram ≤ 2 callbacks).
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** `theme.rs` portando `top_level_field`/`latest_version_dir`/`load` de `notificacoes/src/render/theme.rs` (146 LOC) e `watch.rs` com `inotify` sobre `CLOSE_WRITE | MOVED_TO | CREATE` no diretório pai de cada `vN`, entregue como `calloop::generic::Generic` com debounce de 40 ms via `calloop::timer`. Em erro de parse mantém o tema anterior e loga em `tracing::warn!`.
- [ ] **Step 4: Rodar** → `5 passed`.

---

### Task 6: engine — animação e contagem regressiva

Extrai `notificacoes/src/timing.rs` (`Countdown`, `fade_alpha`) e acrescenta o slide do popup do Central (48 px + fade, 240 ms).

**Files:**
- Create: `crates/engine/src/anim.rs`
- Test: `crates/engine/tests/anim.rs`

**Interfaces:**
- Produces: `anim::ease_out_cubic(t: f32) -> f32`; `anim::fade_alpha(elapsed: Duration, total: Duration) -> f32`; `anim::Slide { distance_px: f32, total: Duration }` com `Slide::at(&self, elapsed: Duration) -> SlideFrame { offset_px: f32, alpha: f32 }`; `anim::Countdown` com `start/pause/resume/restart/remaining/fraction/deadline/is_paused` (mesma API de `notificacoes::timing::Countdown`); `anim::MIN_FRAME: Duration = 16 ms`.

- [ ] **Step 1: Testes:** `ease_out_cubic_endpoints` (0→0, 1→1, 0.5 → 0.875 ± 1e-6); `slide_starts_offset_and_ends_at_rest` (`Slide{48.0, 240ms}`: em 0 → `offset_px==48.0, alpha==0.0`; em 240 ms → `0.0, 1.0`; acima de 240 ms → clamp); `countdown_pause_resume_keeps_remaining` (total 10 s, pausa em t=4 s, resume em t=9 s → `remaining(t=9s)==6s`); `countdown_deadline_none_when_paused`.
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Portar** `Countdown` e `fade_alpha` de `timing.rs` (209 LOC, sem a parte específica de notificações `display_duration`/`next_bar_step`) e implementar `Slide`.
- [ ] **Step 4: Rodar** → `4 passed`.

---

### Task 7: engine — ícones e SVG

Extrai `notificacoes/src/icons.rs` e `fsread.rs` (carregamento seguro sem risco de FIFO).

**Files:**
- Create: `crates/engine/src/icons.rs`, `crates/engine/src/fsread.rs`
- Test: `crates/engine/tests/icons.rs`, fixtures `crates/engine/tests/fixtures/{icon.svg,icon.png}`

**Interfaces:**
- Produces: `icons::IconLoader::new(theme_name: Option<String>) -> IconLoader`; `IconLoader::load(&mut self, name_or_path: &str, size_px: u32) -> Option<tiny_skia::Pixmap>` (nome do tema freedesktop, caminho absoluto PNG/SVG); `fsread::read_regular(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>>` (recusa FIFO/dispositivo e arquivos maiores que `max_bytes`).

- [ ] **Step 1: Testes:** `loads_absolute_svg_at_requested_size` (SVG 24×24 pedido a 48 → pixmap 48×48 e pixel central não transparente); `loads_png`; `unknown_name_returns_none`; `read_regular_rejects_fifo` (cria FIFO com `mkfifo` em tempdir → `Err` de tipo `InvalidInput` sem bloquear; o teste tem timeout de 2 s); `read_regular_rejects_oversized`.
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Portar** `icons.rs` (762 LOC) mantendo só carregamento de ícone por nome/caminho + SVG via resvg; deixar de fora o que é específico de notificações (miniaturas e `.desktop` ficam para as fases 2/4) e portar `fsread.rs` (62 LOC).
- [ ] **Step 4: Rodar** → `5 passed`.

---

### Task 8: engine — superfície Wayland (layer-shell/popup, shm)

**Files:**
- Create: `crates/engine/src/wayland/{mod.rs,surface.rs,pool.rs,output.rs}`
- Create: `examples/surface_smoke.rs` (abre uma barra de 40 px com texto e fecha em 3 s)
- Test: `crates/engine/tests/pool.rs` (lógica pura), verificação manual no compositor aninhado

**Interfaces:**
- Consumes: `theme::Theme`, `text::Text`.
- Produces:
  - `wayland::Anchor { Top, Bottom, Left, Right }`;
  - `wayland::SurfaceKind { Layer { layer: Layer, anchor: Anchor, exclusive_zone: i32, keyboard: KeyboardMode, namespace: String }, Popup { parent: SurfaceId, rect: Rect } }`;
  - `wayland::SurfaceId(u64)`;
  - `wayland::Shell::new(handle: LoopHandle<'_, D>) -> Result<Shell>`; `Shell::create_surface(&mut self, kind: SurfaceKind, output: OutputId, size: (u32,u32)) -> SurfaceId`; `Shell::draw(&mut self, id: SurfaceId, f: impl FnOnce(&mut Pixmap, scale: u32))`; `Shell::destroy(&mut self, id: SurfaceId)`; evento `ShellEvent::{OutputAdded(OutputId), OutputRemoved(OutputId), Configure{id,size,scale}, Closed(id), Pointer(..), Key(..)}`;
  - `pool::BufferPool::slots_needed(width: u32, height: u32, scale: u32) -> usize` e `pool::stride(width_px: u32) -> u32` (= `width_px * 4`).

- [ ] **Step 1: Teste** `pool_stride_and_size_scale_with_buffer_scale` (80×40 com `scale=2` → buffer 160×80, stride 640, tamanho 51200 bytes) e `pool_reuses_released_slot` (solta o slot, pede de novo, o mesmo offset volta; nenhum slot extra é alocado).
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** o `Shell` com sctk 0.20 (wlr_layer, wl_shm, xdg_popup, fractional-scale, cursor-shape, exclusive zone), reaproveitando a lógica de `notificacoes/src/wayland/{surface.rs,mod.rs}` (232 LOC). **Regra extraída do bug das prévias presas:** o estado do popup é sempre derivado de `ShellEvent::Closed/Configure`; nunca de flag local sem validação.
- [ ] **Step 4: Rodar** `cargo test -p engine --test pool` → `2 passed`.
- [ ] **Step 5: Verificação manual no compositor aninhado** (infra da Task 15; até lá, na VM): `cargo run -p engine --example surface_smoke` → a barra de 40 px aparece com o texto e sai em 3 s sem erro de protocolo no log do compositor; reservar exclusive zone muda a área útil das janelas (confirmar visualmente) e some quando a barra fecha.

---

### Task 9: engine — entrada, texto editável e IME

**Files:**
- Create: `crates/engine/src/input/{mod.rs,keyboard.rs,pointer.rs,ime.rs}`, `crates/engine/src/widgets/{textfield.rs,scroll.rs}`
- Test: `crates/engine/tests/textfield.rs`, `crates/engine/tests/keyboard.rs`, `crates/engine/tests/scroll.rs`

**Interfaces:**
- Produces:
  - `input::KeyEvent { keysym: u32, text: Option<String>, mods: Modifiers, pressed: bool, repeat: bool }`; `input::Keyboard::from_keymap(keymap: &str) -> Result<Keyboard>`; `Keyboard::key(&mut self, scancode: u32, pressed: bool) -> KeyEvent`; `Keyboard::repeat_info() -> (delay_ms: u32, rate_per_s: u32)`;
  - `widgets::TextFieldState::new() -> Self`; `insert(&mut self, s: &str)`; `backspace(&mut self)`; `delete(&mut self)`; `move_cursor(&mut self, by: CursorMove, extend: bool)`; `set_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>)`; `commit_preedit(&mut self, text: &str)`; `text(&self) -> &str`; `cursor(&self) -> usize` (byte offset em fronteira de grafema); `selection(&self) -> Option<Range<usize>>`;
  - `widgets::ScrollState::new(content_h: f32, viewport_h: f32) -> Self`; `scroll_by(&mut self, dy: f32)`; `offset(&self) -> f32` sempre em `[0, max(0, content_h − viewport_h)]`;
  - `input::ime::TextInputV3` (ligação ao protocolo; eventos `Preedit/Commit/DeleteSurrounding/Done`).

- [ ] **Step 1: Testes do campo de texto** (puros):
  - `insert_and_backspace_by_grapheme` (insere `"e\u{301}"` e depois backspace remove os dois escalares de uma vez; insere `"👨‍👩‍👧"` e backspace remove o cluster inteiro);
  - `cursor_never_inside_cluster` (**Review Focus 4**): para a string `"a👨‍👩‍👧b"` percorre `move_cursor(Right)` e asserta que `cursor()` só assume offsets `{0, 1, 19, 20}` (offsets em bytes dessa string);
  - `preedit_is_not_in_text_until_committed`: `set_preedit("にほん", Some((3,3)))` deixa `text()==""`; `commit_preedit("日本")` → `text()=="日本"` e preedit vazio;
  - `selection_replaced_on_insert`.
- [ ] **Step 2: Testes do teclado/scroll:** `keyboard_maps_keysym_and_text_for_us_layout` (keymap US embutido em fixture: scancode da tecla `a` → keysym `0x61` e `text==Some("a")`; com Shift → `"A"`); `dead_key_compose_yields_accented_char` (compose `´` + `e` → `"é"`); `scroll_offset_clamps` (content 500, viewport 200: `scroll_by(1000)` → 300; `scroll_by(-1000)` → 0; conteúdo menor que o viewport → sempre 0).
- [ ] **Step 3: Rodar** → FAIL.
- [ ] **Step 4: Implementar** com `xkbcommon` (keymap + compose) e `unicode-segmentation` (grafemas); `TextInputV3` via `wayland-protocols` (`zwp_text_input_v3`); repeat de tecla é computado pelo `Keyboard` mas **agendado por timer único do calloop** (sem thread).
- [ ] **Step 5: Rodar** `cargo test -p engine` (tudo) → todos passam; contagem impressa no relatório.

---

### Task 10: engine — acessibilidade (AccessKit)

**Files:**
- Create: `crates/engine/src/a11y.rs`
- Test: `crates/engine/tests/a11y.rs`

**Interfaces:**
- Produces: `a11y::NodeKind { Window, Button, Label, TextInput, List, ListItem }`; `a11y::A11yTree::new() -> Self`; `A11yTree::add(&mut self, parent: Option<NodeId>, kind: NodeKind, label: &str, bounds: Rect) -> NodeId`; `A11yTree::set_focus(&mut self, id: NodeId)`; `A11yTree::to_update(&self) -> accesskit::TreeUpdate`.

- [ ] **Step 1: Testes:** `tree_update_has_role_and_label` (um `Button "Desligar"` vira nó com `Role::Button` e nome `"Desligar"`); `focus_is_exposed` (`set_focus` → `TreeUpdate.focus == id`); `text_input_exposes_value` (campo com `"abc"` → `Role::TextInput` com valor `"abc"`).
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** o mapeamento sobre `accesskit` (confirmar a versão publicada e o adaptador para Wayland/AT-SPI com `cargo search accesskit`; se o adaptador unix exigir runtime assíncrono, usar um thread dedicado fora do loop de render).
- [ ] **Step 4: Rodar** → `3 passed`.

---

### Task 11: core — trait `Module` e supervisor

**Files:**
- Create: `crates/core/src/module.rs`, `crates/core/src/supervisor.rs`
- Test: `crates/core/tests/supervisor.rs`

**Interfaces:**
- Produces:
  - `module::ModuleId(&'static str)`;
  - `trait Module: Send { fn id(&self) -> ModuleId; fn start(&mut self, ctx: &mut ModuleCtx) -> Result<(), ModuleError>; fn stop(&mut self); fn on_config(&mut self, cfg: &toml::Table) -> Result<(), ModuleError>; fn heartbeat(&self) -> Option<Instant>; }`;
  - `module::ModuleCtx { shell: &mut engine::wayland::Shell, theme: engine::theme::Theme, bus: BusHandle }`;
  - `supervisor::Supervisor::new(policy: RestartPolicy) -> Self`; `register(&mut self, factory: Box<dyn Fn() -> Box<dyn Module> + Send>)`; `enable(&mut self, id: ModuleId)`; `disable(&mut self, id: ModuleId)`; `status(&self, id: ModuleId) -> ModuleState` com `ModuleState::{Disabled, Running, Restarting{attempt: u32, next_in: Duration}, Failed}`; `RestartPolicy { base: Duration, max: Duration, max_attempts: u32, heartbeat_timeout: Duration }`.

- [ ] **Step 1: Testes** (**Review Focus 3**), com módulos falsos:
  - `panic_in_start_restarts_only_that_module` (módulo A paniquea em `start`, B está saudável; A fica `Restarting{attempt:1}`, B continua `Running`; o processo de teste segue vivo);
  - `backoff_grows_and_caps` (3 falhas seguidas: espera `base`, `2*base`, `4*base` até o `max`);
  - `hung_module_detected_by_heartbeat` (módulo que para de atualizar `heartbeat` por > `heartbeat_timeout` é marcado e reiniciado);
  - `too_many_failures_marks_failed` (`max_attempts` excedido → `Failed`, sem novas tentativas até `enable` explícito);
  - `disable_stops_and_calls_stop_once`.
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** cada módulo na sua thread com `std::panic::catch_unwind`; heartbeat vigiado por um único timer do calloop (sem polling). Thread presa não pode ser morta dentro do processo: o supervisor a marca abandonada e cria nova instância (limitação documentada: módulos de risco vão para subprocesso, spec §3.1.1.3).
- [ ] **Step 4: Rodar** `cargo test -p core` → `5 passed`.

---

### Task 12: core — config, recarga a quente e liga/desliga

**Files:**
- Create: `crates/core/src/config.rs`
- Test: `crates/core/tests/config.rs`

**Interfaces:**
- Consumes: `supervisor::Supervisor`, `engine::watch`.
- Produces: `config::ConfigStore::open(dir: PathBuf) -> Result<ConfigStore>`; `ConfigStore::module(&self, id: ModuleId) -> toml::Table`; `ConfigStore::set_enabled(&mut self, id: ModuleId, on: bool) -> Result<()>` (grava atomicamente: arquivo temporário + `rename`); `ConfigStore::watch(&self, handle, on_change: impl FnMut(ConfigChange))` com `ConfigChange::{Enabled(ModuleId, bool), Module(ModuleId, toml::Table), Invalid{file: PathBuf, error: String}}`; arquivo principal `<dir>/cosmic-cosmetics.toml` com `[modules.<id>] enabled = true|false` e tabela por módulo em `<dir>/modules/<id>.toml`.

- [ ] **Step 1: Testes:**
  - `set_enabled_writes_atomically` (durante a escrita nunca existe arquivo parcial: simular leitor concorrente que sempre vê TOML válido, 200 iterações);
  - `invalid_toml_keeps_previous_config` (**Review Focus 2**): arquivo é truncado no meio de uma chave; o `watch` emite `Invalid{…}` e `module()` continua devolvendo a tabela anterior;
  - `enable_disable_event_reaches_supervisor` (editar o arquivo para `enabled=false` produz `Enabled(id,false)` em < 200 ms e o supervisor desliga o módulo sem reiniciar o processo);
  - `unknown_module_in_file_is_ignored_with_warning`.
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** com `toml` + `serde`, reaproveitando o `watch.rs` do engine (Task 5).
- [ ] **Step 4: Rodar** → `4 passed`.

---

### Task 13: holder — zona exclusiva sobrevive ao daemon

**Files:**
- Create: `bin/holder/src/{main.rs,zone.rs,handover.rs}`
- Test: `bin/holder/tests/state.rs` (máquina de estados pura), verificação manual no compositor aninhado

**Interfaces:**
- Consumes: `engine::wayland::{Shell, SurfaceKind, Anchor}`.
- Produces:
  - `zone::ZoneSpec { output: OutputId, anchor: Anchor, size_px: u32 }`;
  - `zone::HolderState::{Idle, Holding(Vec<ZoneSpec>)}` e `zone::reconcile(state: &HolderState, outputs: &[OutputId], desired: &DesiredZones) -> Vec<ZoneAction>` com `ZoneAction::{Create(ZoneSpec), Destroy(OutputId), Keep}`;
  - D-Bus de sessão `io.github.Aenes0.CosmicCosmetics.Holder`: método `SetZones(a(suus))` (daemon declara as zonas desejadas), sinal `ZonesChanged`; o holder **só** reserva a zona e entrega ao daemon um `wl_surface` compartilhado **ou** só mantém a reserva (**decisão do builder** conforme spec §3.1.1.2, registrar a escolha em `bin/holder/README.md`).

- [ ] **Step 1: Testes puros** (**Review Focus 5**):
  - `reconcile_keeps_zone_when_daemon_restarts` (holder em `Holding([zona(DP-3)])`, daemon some e volta pedindo as mesmas zonas → `[Keep]`, nenhum `Destroy`);
  - `reconcile_creates_zone_for_new_output` (hotplug de `DP-4` → `[Create(zona DP-4)]`);
  - `reconcile_destroys_zone_when_output_removed` (saída sumiu → `Destroy(DP-3)`; nenhuma ação referencia o output removido);
  - `reconcile_ignores_zone_for_unknown_output` (daemon pede zona num output que o compositor nunca anunciou → `[]`).
- [ ] **Step 2: Rodar** → FAIL.
- [ ] **Step 3: Implementar** `zone::reconcile` (função pura) e o `main` do holder com `engine::wayland::Shell`; o holder não depende de `core` nem de módulos; RSS alvo a medir na Task 15.
- [ ] **Step 4: Rodar** `cargo test -p cosmic-cosmetics-holder` → `4 passed`.
- [ ] **Step 5: Manual (compositor aninhado):** subir holder + daemon "hello"; `kill -9` o daemon; **a área útil das janelas não muda** (as janelas de teste não se movem) e o systemd recoloca o daemon; repetir com desconexão de uma saída virtual.

---

### Task 14: units systemd e cgroup

**Files:**
- Create: `packaging/systemd/cosmic-cosmetics-holder.service`, `packaging/systemd/cosmic-cosmeticsd.service`, `packaging/systemd/cosmic-cosmetics-fallback.service`
- Create: `scripts/verify-units.sh`
- Test: `scripts/verify-units.sh` (usa `systemd-analyze --user verify`)

**Interfaces:**
- Produces: holder com `Restart=always`, `WatchdogSec` e sem prioridade elevada (nunca `Nice` negativo); daemon com `Requires=cosmic-cosmetics-holder.service`, `Restart=on-failure`, `RestartSec=100ms`, `WatchdogSec=5s`, `CPUWeight=50`, `IOWeight=50`, `MemoryHigh` e `Nice=5` (**valores iniciais**, a validar com a medição, spec §3.1); `fallback.service` ativado por `OnFailure=` do daemon, que inicia `cosmic-panel`/`cosmic-notifications` originais (a lógica N-falhas/janela é do plano da fase 9; aqui só a unit com um `StartLimit` de exemplo).

- [ ] **Step 1: Teste:** `scripts/verify-units.sh` falha se `systemd-analyze --user verify packaging/systemd/*.service` imprimir qualquer aviso ou erro.
- [ ] **Step 2: Rodar** → FAIL (units não existem).
- [ ] **Step 3: Escrever as três units** com os campos acima; `ConditionEnvironment=WAYLAND_DISPLAY`, `NoNewPrivileges=yes`, `PartOf=graphical-session.target`.
- [ ] **Step 4: Rodar** `scripts/verify-units.sh` → saída vazia e exit 0.

---

### Task 15: daemon "hello", compositor aninhado e portão da fase

**Files:**
- Create: `bin/daemon/src/main.rs` (liga `core` + `engine`), `crates/core/src/modules_hello.rs` (módulo `hello`: uma barra com relógio, `heartbeat` real), `scripts/nested-comp.sh` (sobe `cosmic-comp` aninhado em janela), `docs/testes/AMBIENTE.md` (como usar o aninhado e a VM)
- Test: `bin/daemon/tests/hot_toggle.rs`, `scripts/measure-idle.sh`

**Interfaces:**
- Consumes: tudo das tarefas 3–14.
- Produces: `cosmic-cosmeticsd` que lê a config, registra `hello` no supervisor, aplica o tema ao vivo e responde a `enabled=false/true` sem reiniciar o processo; `scripts/measure-idle.sh <segundos>` imprime wakeups/s e CPU % do holder e do daemon, e RSS/PSS de cada um.

- [ ] **Step 1: Teste** `hello_module_toggles_without_process_restart`: sobe o `Supervisor` + `ConfigStore` em tempdir com `hello` ligado, anota o PID e o `Instant` de início do processo, edita a config para `enabled=false`, espera `ModuleState::Disabled`, volta para `true`, espera `Running`; asserta que o PID é o mesmo e que o contador de superfícies criadas é 1, destruídas 1, recriadas 1.
- [ ] **Step 2: Rodar** `cargo test -p cosmic-cosmeticsd` → FAIL.
- [ ] **Step 3: Implementar o daemon e o módulo `hello`** (barra de 28 px com o relógio de `HH:MM` redesenhado só quando o minuto muda, via timer único).
- [ ] **Step 4: Rodar** → `1 passed`.
- [ ] **Step 5: Ambiente aninhado:** `HOST 'cd ~/Projetos/cosmic-cosmetics && scripts/nested-comp.sh'` abre a janela do compositor; dentro dela rodar holder + daemon. Verificar visualmente: barra aparece, tema muda ao alternar claro/escuro nas configurações do COSMIC aninhado, `kill -9` do daemon não move as janelas de teste.
- [ ] **Step 6: Portão da fase (evidência obrigatória, G3):**
  - `cargo test --workspace` → contagem total impressa, 0 falhas;
  - `scripts/measure-idle.sh 60` com holder+daemon+hello em repouso: `wakeups/s` e `CPU %` ≈ 0 (valores registrados em `docs/baseline/depois-fase1.json`); holder RSS e daemon RSS registrados;
  - `scripts/verify-units.sh` → exit 0.
  Registrar qualquer divergência do plano (ex.: API de `accesskit`, versão de `sctk`) na seção "Divergências" no fim deste arquivo.

## Divergências do plano (preencher durante a execução)
Fonte: relatórios em `~/Projetos/cosmic-cosmetics/.superpowers/sdd/task-*-report.md` e rulings R1–R11
(`rulings.md`). Preenchido na Task 15 (2026-10-23).

**Processo e nomes**
- R1/R8: nenhum commit; revisões por snapshot/diff do working tree (sem git no sandbox).
- R2: pacotes `cosmic-cosmetics-engine` (lib `engine`), `cosmic-cosmetics-core` (lib `cosmic_core`),
  `cosmic-cosmetics-holder`, `cosmic-cosmeticsd`; onde o plano diz `-p engine`/`-p core`, valem esses nomes.
  Holder e daemon ganharam `[lib]` (partes puras testáveis) além do binário.
- R3: todo cargo/python roda no host via `flatpak-spawn --host`.

**Engine**
- Task 1: versões escolhidas `xkbcommon 0.9`, `accesskit 0.25`, `inotify 0.11`, `toml 1`; sem `rust-toolchain.toml`.
- Task 4 (R10): `Laid.height`/`width` são px **lógicos**; o teste `layout_scale_doubles_height` do plano estava errado.
- Task 5 (R6/R11): `ThemeWatcher` construído sobre um `engine::watch::DirWatcher` genérico (reusado pela
  config); `ThemeWatcher::new` devolve `io::Result`; debounce só *trailing*; lê também raio
  (`radius_m`) e espaçamento (`space_xxs`) com FALLBACK campo a campo; `top_level_field`/`latest_version_dir`
  passaram a `engine::theme`. Task 12: `DirWatcher<'l>`/`ThemeWatcher<'l>` ganharam o lifetime do laço.
- Task 5/8: `smithay-client-toolkit 0.20` com `default-features = false, features = ["calloop"]` — já traz
  wlr-layer, xdg-popup, cursor-shape e os protocolos staging (fractional-scale, viewporter); sem duplicatas.
- Task 8: `Shell::new(&LoopHandle, on_event: FnMut(&mut D, ShellEvent))` (callback de eventos) em vez de
  `new(handle)`; `ShellHandle` (`Clone + Send`, R5) com `create_surface/draw/destroy` e `draw` recebendo
  `FnOnce(&mut Pixmap, u32) + Send + 'static`; pool shm próprio sobre `RawPool` (2 slots, `checked_pool_len`);
  escala fracionária desenha na inteira de cima e reduz pelo viewport; sem `xdg_popup.grab`; exemplo em
  `crates/engine/examples/surface_smoke.rs`. Task 11: `ShellHandle::detached()` (inerte, para headless/testes).
- Task 9: `Keyboard::from_keymap_with_locale`, `key()` não arma a repetição sozinho (o consumidor usa
  `RepeatTimer`), `TextInputV3::reset()`; pendente M1 (`set_content_type` — obrigatório antes de campos de senha).
- Task 10: accesskit 0.25.1 — `Tree` está deprecado, usa `TreeInfo` (`TreeUpdate { nodes, tree:
  Option<TreeInfo>, tree_id, focus }`); `A11yTree::set_value` acrescentado; árvore auto-corretiva (sem panics).
- Task 13/15: o sd_notify do holder virou `engine::sdnotify` (com `start_watchdog` no calloop), usado pelo
  holder e pelo daemon.

**Core**
- Task 11 (R5): `ModuleCtx { shell: ShellHandle, theme, bus, live }` — sem `&mut Shell`; módulos em threads;
  timer do supervisor é `recv_timeout` numa thread monitora (core sem calloop), ratificado; API extra
  (`with_context`, `configure`, `last_error`, `subscribe`, `live_threads`); sem estado `Starting`
  (durante o 1º start o estado é `Disabled`).
- Task 12: `ConfigStore::register(id, default)` (API extra); Task 15 acrescentou `try_register` e recusa
  registro com `watch` ativo (achado M1).
- Task 15: `cosmic_core::live` (`LiveCtx`: tema/saídas atuais + `Broadcast<LiveEvent>`) — é como os módulos
  vivos recebem tema e eventos do `Shell` (o ctx é copiado no `start`). O `hello` ficou em
  `crates/core/src/modules_hello.rs` como o plano pede (testes em `modules_hello_tests.rs`).

**Holder / daemon / empacotamento**
- Task 13: o holder **só reserva** a zona (layer Bottom transparente, 1 px ao longo da borda); o daemon
  desenha em layer Top com `exclusive_zone = -1`; `SetZones(a(suus))` por nome de conector; persistência em
  `$XDG_RUNTIME_DIR/cosmic-cosmetics/holder-zones`; contrato de reenvio (NameOwnerChanged/ZonesChanged).
- Task 14 (R7): `verify-units.sh` verifica cópias com `ExecStart=/usr/bin/true`; holder com
  `StartLimitIntervalSec=30s/Burst=10 + OnFailure=fallback`. ⚠️ o fallback inicia painel/notificações por
  unit, mas o cosmic-session os lança como filhos — testar numa sessão real/VM.
- Task 15: o daemon observa a config com `ConfigStore::watch` + `apply::apply_change` (o mesmo que
  `apply::watch_supervisor`) para também recalcular as zonas pedidas ao holder; zona só com
  `reserve = true` + `output` em `modules/hello.toml` (padrão: nenhuma).
- Task 15: `blocking` (transitivo do zbus) fixado em `~1.6.2`: a 1.7.0 mantém uma thread do pool acordando
  a cada 500 ms para sempre (medido: 2,2 wakeups/s em repouso → 0,25 com a 1.6.2).
- Task 15: `cosmic-comp` não tem flag de aninhamento; com `WAYLAND_DISPLAY` ele escolhe sozinho o backend
  winit. `scripts/nested-comp.sh` usa isso + `dbus-run-session` + config isolada.
- Task 15, passo 6: a medição registrada em `docs/baseline/depois-fase1.json` é **HEADLESS** (daemon sem
  Wayland, sem holder); a medição com GUI (holder + daemon + barra) segue pendente (R4).
