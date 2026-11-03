# Central e logi-osd no cosmic-taskbar: plano

> Ninguém commita sem pedido do usuário; subagentes recebem "não commite". Spec:
> `docs/superpowers/specs/2026-11-03-central-logi-design.md`.

**Base antes da fusão (medir no início da Fase 2):** PSS, threads e fds de cada processo do Central
(applets `--monitor`, `--tray`, `--show-desktop`, principal; serviços `--panels`,
`--notifications`, `--clipboard`) e do `logi-osd`.

## Fase 0: preparo (feita)

- [x] Arquivar `cosmic-ext-applet-tray` e `notificacoes` com aviso.
- [x] Commitar o pendente do Central (`e2f17ee`: super-economia por unidades, DIAG da bandeja, planos).

## Fase 1: importar com histórico

- [ ] 1.1 `git subtree add --prefix=import/central ../quick-settings main` e `--prefix=import/logi ../logi-osd main`.
- [ ] 1.2 `git mv` para `src/central/` e `src/logi/` (o `main.rs` de cada um vira `mod.rs` com `pub fn run()`); dados para `data/central/` e `data/logi/`; docs antigos para `docs/historico/{central,logi}`; fixtures de teste junto.
- [ ] 1.3 Reescrever `crate::` → `crate::central::` / `crate::logi::` (inclusive `$crate` em macros e caminhos de `include_*!`).
- [ ] 1.4 Despacho em `src/main.rs`: subcomandos `central` e `logi`; nomes antigos `cosmic-ext-applet-central` e `logi-osd` (por argv[0]). Teste de `mode()` para cada caso, inclusive `--show-desktop` nos dois sentidos. Cada modo aplica seu `TOKIO_WORKER_THREADS` antes de qualquer thread.
- [ ] 1.5 `Cargo.toml`: união das dependências (crates do cosmic-settings `ae5c750`, `serde_json`, `image` com jpeg/png/webp, `udev`, `dbus`, `calloop`, `rustix` com `event`), sem GPU, libcosmic `5a8bd94`, cosmic-protocols `c0cff4d`. `cargo update -p libcosmic --precise 5a8bd94…` se o lock andar.

## Fase 2: compilar e testar

- [ ] 2.1 `cargo build --release` até compilar; registrar cada mudança obrigada pelo libcosmic novo (arquivo e motivo).
- [ ] 2.2 `cargo test`: meta = os testes das três bases (≈ 482 + 219 + 225) passando. Testes do logi-osd que usam `dbus-daemon` real continuam (rodam com o daemon do sistema).
- [ ] 2.3 Medir a base (acima) com os binários antigos ainda instalados.

## Fase 3: instalar

- [ ] 3.1 `install.sh` único: binário, links dos nomes antigos (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`, `cosmic-ext-applet-central`, `logi-osd`), os 6 `.desktop`, as 4 unidades do systemd (`central-panels`, `central-notifications`, `central-clipboard`, `logi-osd`) com `ExecStart=… cosmic-taskbar central --panels` etc. e os mesmos limites, os arquivos de ativação D-Bus, `daemon-reload` e reinício das unidades.
- [ ] 3.2 Instalar com o OK do usuário (o painel e os serviços reiniciam).
- [ ] 3.3 Verificação na tela da spec, com o usuário. Medir de novo e comparar.

## Fase 4: reaproveitar (só depois da Fase 3 verde)

- [ ] 4.1 `shared::logging::init(default_filter, quiet_targets)` no lugar das 3 cópias (barra, menu, Central) e do logi-osd; mesmo comportamento de cada um.
- [ ] 4.2 `shared::locale` devolve o idioma (`pt`, `fr`, `en`); o i18n do Central usa.
- [ ] 4.3 Revisão: há mais duplicação real? (cosmic-config de layout/espaçamento, `COSMIC_PANEL_OUTPUT`). Só fazer o que der ganho.

## Fase 5: fechar

- [ ] 5.1 README e gaps atualizados.
- [ ] 5.2 Com o OK do usuário: aviso e arquivamento de `cosmic-ext-applet-central` e `logi-osd`.
- [ ] 5.3 Revisão final (`revisor`) do diff.

## Review Focus

1. Os painéis carregam o histórico de notificações igual a hoje (a correção recente é o que mais pode regredir com o libcosmic novo).
2. Um modo que trava não leva outro junto (unidades e processos separados, watchdog do `--panels`).
3. Os nomes antigos (`cosmic-ext-applet-central --panels`, `logi-osd`) ainda funcionam pelas unidades velhas durante a troca.
4. Nenhuma biblioteca de GPU carregada em nenhum processo.
5. O `--show-desktop` do menu e o do Central chegam cada um à sua parte.
