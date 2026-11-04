# Canto único: plano

> **Para os executores:** cada tarefa é de um subagente; não commite; TDD; licença de corrigir o
> plano (se a assinatura daqui não bater com o código real, ajuste e relate).

**Objetivo:** bandeja, Central e mostrar área num applet só (`central --corner`), um processo por tela.
**Arquitetura:** um invólucro `Corner` no molde de `src/central/panels/mod.rs` (enum de mensagens +
`lift`), com os três apps como filhos; cada filho ganha um modo "no canto" que tira o autosize e
troca o clique/passagem da janela inteira por clique/passagem na sua área.
**Spec:** `docs/superpowers/specs/2026-11-03-canto-unico-design.md`.
**Testes:** `flatpak-spawn --host sh -c "cd ~/Projetos/cosmic-taskbar && cargo test <filtro>"`.

## Restrições globais

- Os modos sozinhos (`central`, `--tray`, `--show-desktop`) não mudam de comportamento.
- Ordem no canto: bandeja, Central, mostrar área. APP_ID `io.github.Aenes0.CosmicCentral.Corner`.
- Nada de zoom/prévias; não mexer no monitor de uso.

## Foco de revisão

1. Clique na bandeja não abre o Central nem esconde janelas (teste de unidade em cada filho: no
   canto, a assinatura não escuta `ButtonPressed` da janela).
2. Clique que fechou o painel do Central não o reabre (regra dos 250 ms preservada no clique por área).
3. Passar o mouse sobre o Central não dispara a espiada do mostrar área.
4. `core` dos filhos acompanha o do invólucro (tema, tamanho do painel): sincronizar antes de cada update.
5. `setup.sh` idempotente: rodar duas vezes não duplica; `--undo` devolve os 3 ids no lugar.

---

### Onda 1 (paralela, arquivos disjuntos)

### Tarefa 1: Central no canto — `src/central/app.rs`
- Produz: `pub(crate) fn in_corner(core: app::Core) -> (App, app::Task<Message>)` (como `init`,
  com `corner = true`); `pub(crate) fn corner_view(&self) -> Element<'_, Message>` (o botão de hoje,
  sem `autosize_window`, com o clique por área disparando a mesma lógica do `PanelPress`, inclusive
  `just_closed`); no canto, `subscription()` não inclui o `listen_with` de `ButtonPressed`.
- [x] Teste: `corner_does_not_listen_to_window_presses` (ou equivalente pela função que decide).
- [x] Teste: o clique por área dentro de 250 ms do fechamento não reabre.

### Tarefa 2: Mostrar área no canto — `src/central/show_desktop.rs`
- Produz: `pub(crate) fn in_corner(core) -> (ShowDesktop, Task<Message>)`, `pub(crate) fn
  corner_view(&self) -> Element<'_, Message>` (botão sem autosize, `mouse_area` com
  `on_press(Click)`, `on_enter(Enter)`, `on_exit(Leave)`); no canto, a assinatura mantém a thread
  Wayland e tira `ButtonPressed`/`CursorEntered`/`CursorLeft`.
- [x] Teste: no canto, os eventos da janela não viram `Press`/`Hover`.

### Tarefa 3: Bandeja no canto — `src/central/tray/components/{app,view,drag}.rs`, `tray/mod.rs`
- Produz: `pub(crate) fn in_corner(core) -> (App, Task<Msg>)`, `pub(crate) fn corner_view(&self)
  -> cosmic::Element<'_, Msg>` (a `line` sem autosize), `pub(crate) fn set_reserved(&mut self,
  px: f32)` (descontado do `suggested_bounds` antes de `layout::capacity`); no canto,
  `resize_window()` devolve `Task::none()` (quem dimensiona é o autosize do invólucro). Tornar
  `components`/`app::App`/`Msg` visíveis em `crate::central` (`pub(crate)`).
- [x] Teste: capacidade com `reserved` desconta (ex.: 300 px, botão 30 px, reserved 90 → 7).
- [x] Teste: no canto `resize_window` não redimensiona.

### Tarefa 4: Instalação e painel — `data/central/…Corner.desktop.in`, `install.sh`, `setup.sh`
- `.desktop`: `Exec=@bindir@/cosmic-taskbar central --corner`, `X-CosmicApplet=true`,
  `X-HostWaylandDisplay=true`, `X-CosmicHoverPopup=Auto`, `X-OverflowPriority=99`,
  `X-OverflowMinSize=2`, `NoDisplay=true`, nomes "Corner"/"Canto".
- `install.sh` instala o novo `.desktop` junto dos outros do Central.
- `setup.sh`: em `plugins_wings`, se os 3 ids estão presentes e seguidos (`CosmicTray`,
  `CosmicCentral`, `CosmicCentral.ShowDesktop`), troca pelos `…Corner`; se não, avisa e não
  mexe; idempotente. `--undo` faz o inverso.
- [x] Teste: script de teste em `tests/setup_corner.sh` sobre uma cópia temporária de
  `plugins_wings` (ida, segunda ida sem mudança, volta).

### Onda 2

### Tarefa 5: Invólucro — `src/central/corner.rs`, `src/central/mod.rs`
- `Corner { core, tray, central, show }`, `enum Message { Tray(Msg), Central(app::Message),
  Show(show_desktop::Message) }`, `lift` como em `panels/mod.rs:151` (mover para um lugar comum ou
  duplicar 8 linhas). `init` chama os três `in_corner(core.clone())`; `update` sincroniza o `core`
  dos filhos e delega; `view` = `autosize_window(row/column [tray, central, show])`;
  `view_window`/`on_close_requested` vão para a bandeja (os popups são dela); `style` =
  `cosmic::applet::style()`; `subscription` = batch mapeado. Em `mod.rs`, `--corner` antes dos
  outros modos.
- [x] Teste: mensagens chegam ao filho certo; `set_reserved` recebe a largura do Central + mostrar
  área.

### Tarefa 6: Verificar

> Executado 03/11. Divergências: `in_corner` do Central e do mostrar área são funções livres; o
> clique no canto é `on_press_down` do próprio botão (um `mouse_area` em volta nunca o veria: o
> botão captura o aperto, `iced/widget/src/mouse_area.rs:278`); o `mouse_area` do mostrar área
> fica só para entrada/saída. A folga da bandeja é estimada (mostrar área 10 px + 4 botões).
> O teste "mensagens chegam ao filho certo" não foi escrito (precisa de um `Core` de painel).
> Achados depois: (1) os menus da bandeja vêm como `AppPopup`, com ajustes tipados para a
> bandeja que o `map` não converte; o canto os reembrulha (`corner::tray_surface`, teste
> `a_tray_popup_opens_on_the_corner`). (2) Instalado, o socket privilegiado do painel
> (`X_PRIVILEGED_WAYLAND_SOCKET`, um descritor) era aberto por três (áreas de trabalho, mostrar
> área, token do libcosmic): "Invalid new_id", "Bad file descriptor". Agora `central::privileged`
> o tira do ambiente no início e o divide numa conexão só; o token cai no `WAYLAND_DISPLAY`.
> Medido instalado (03/11 22:17): 3 `central --corner`, 9,5 MB e 14 threads cada = 28,5 MB
> (antes: 9 processos, 68 MB, 72 threads).
- [x] `cargo test` verde com contagem; `cargo build --release`.
- [ ] Com OK do usuário: `install.sh` + `setup.sh`; `ps`: 3 `central --corner`, sem os 3 antigos;
  PSS comparado com 68 MB; na tela, menus, ^, arrastar, relógio e mostrar área nas três telas.
