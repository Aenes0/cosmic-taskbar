# Bandeja reconstruída: plano

> Executores: não commite; TDD; licença de corrigir o plano (se uma assinatura daqui não bater com
> o código real, ajuste e relate). Spec: `docs/superpowers/specs/2026-11-03-bandeja-reconstruida-design.md`.
> Testes: `flatpak-spawn --host sh -c "cd ~/Projetos/cosmic-taskbar && cargo test central::tray"`.

**Arquitetura:** uma máquina de estados pura (`tray/components/popups.rs`, sem Wayland) decide o
que pedir e o que destruir; o `App` da bandeja só executa os comandos dela e lhe repassa os fatos
(`Opened` do id, fechamento, tempo). O menu de um ícone da caixa tem pai = caixa.

## Foco de revisão

1. Dois cliques rápidos no ^ (o segundo antes do `Opened`): não pode ficar "aberto" sem popup.
2. Pedido perdido (nunca chega `Opened`): em ~300 ms volta a fechado; o clique seguinte abre.
3. Fechamento pelo compositor (clique fora): fecha menu e caixa; nenhum destroy depois disso.
4. Destruir a caixa com menu filho: o menu também sai do estado, sem esperar um fechamento dele.
5. `Opened` atrasado de um pedido dado como perdido: adotado se nada foi pedido depois; senão destruído.

---

## Onda 1 (paralela)

### Tarefa 1: máquina de estados — `src/central/tray/components/popups.rs` (novo) + `mod popups;` em `components/mod.rs`
- Produz (nomes usados na Tarefa 3):
  - `pub enum Want { Box, Menu { item: usize, in_box: bool } }` (Clone, Copy, Debug, PartialEq)
  - `pub enum Parent { Main, Box(window::Id) }`
  - `pub enum Cmd { Open { want: Want, id: window::Id, parent: Parent }, Destroy(window::Id) }`
  - `pub struct Popups` (Default) com:
    `toggle_box(&mut self, now: Instant, fresh: window::Id) -> Vec<Cmd>`;
    `menu(&mut self, item: usize, in_box: bool, now: Instant, fresh: window::Id) -> Vec<Cmd>`
    (clique direito; mesmo item aberto = fecha);
    `escape(&mut self, now: Instant) -> Vec<Cmd>` (fecha só o menu);
    `opened(&mut self, id: window::Id, now: Instant) -> Vec<Cmd>`;
    `closed(&mut self, id: window::Id, now: Instant, fresh: window::Id) -> Vec<Cmd>`;
    `tick(&mut self, now: Instant, fresh: window::Id) -> Vec<Cmd>`;
    `box_id(&self) -> Option<window::Id>`, `menu_item(&self) -> Option<(usize, window::Id)>`
    (só os *abertos ou pedidos*, para `view_window`), `is_box_open(&self) -> bool`,
    `pending(&self) -> bool` (há pedido ou fechamento em curso; para o `tick`).
  - `pub const LOST: Duration = Duration::from_millis(300);`
- Regras (da spec): fases *pedido* → *aberto* → *fechando*; decidir pelo confirmado; nunca
  `Destroy` de id não aberto; nunca `Open` com algo *fechando* (fica numa fila de 1, a última
  vontade vence; sai no `closed`); *pedido* sem `opened` em `LOST` → perdido; *fechando* sem
  `closed` em `LOST` → dado como fechado; `Opened` de um perdido → adotado se nada foi pedido
  depois, senão `Destroy`. Destruir a caixa marca o menu filho como fechado junto. Menu da barra
  com caixa aberta → `Destroy(caixa)` e o menu na fila. Menu da caixa só com a caixa *aberta*,
  pai `Parent::Box(id)`. `toggle_box` com a caixa *pedida* não faz nada.
- [ ] Testes (antes): um por regra e os 5 do Foco de revisão, com `Instant` simulado.

### Tarefa 2: correções do oficial — `src/central/tray/subscriptions/**`, `components/status_menu.rs`
- Upstream: pop-os/cosmic-applets, pasta `cosmic-applet-status-area` (baixar com curl da API do
  GitHub; `gh` não está instalado). Para cada commit, conferir se já temos; aplicar o que falta:
  `a502720a` (menu atualiza em PropertyUpdate), `6e99ad42` (item duplicado no watcher),
  `bd0d1804` (`unwrap` → `break`; achar o equivalente do `unique_names.rs`), `63292ab7`
  (icon-data do menu em 14 px), `d73ef7dc` (ícones no theme path do app); confirmar
  `c8104a98`, `ebe18a17`, `fddf3d56` (parecem presentes).
- Não tocar `components/app.rs`, `view.rs`, `drag.rs` (Tarefa 3).
- [ ] Um teste por correção onde der (parse/dedupe/puro); relatar as que não têm teste possível.

## Onda 2

### Tarefa 3: ligar a máquina — `components/app.rs`, `view.rs`, `drag.rs`
- `App` troca `popup`/`overflow_popup`/`open_menu`/`box_dismissed_at` por `popups: Popups`;
  executa `Cmd::Open` com `app_popup` (id e pai já decididos, nada de estado na closure) e
  `Cmd::Destroy` com `destroy_popup`; escuta `window::Event::Opened` por id (`listen_with`) →
  `opened`; `on_close_requested`/`Closed` → `closed`; um tick de 100 ms só enquanto `pending()`.
- Menu da caixa: `anchor_rect` relativo à superfície da caixa (linha/coluna de `BOX_COLUMNS`,
  padding 8). Sem trocar de menu no hover dentro da caixa; Esc → `escape`.
- [ ] `cargo test` verde; `cargo build --release`.

### Tarefa 4: verificar na tela (com OK do usuário)
- [ ] Instalar; `reiniciar-painel`; 20 cliques no ^; menu sobre a caixa; arrastar nos 3 sentidos;
  log sem `Destroyed popup does not exist`/`ParentMissing` da bandeja.
