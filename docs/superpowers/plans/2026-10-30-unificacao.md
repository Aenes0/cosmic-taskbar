# Unificação barra + menu Iniciar: plano

> **Para quem executa:** subagent-driven-development ou executing-plans. Passos com `- [ ]`. Ninguém commita sem pedido explícito do usuário; subagentes recebem "não commite".

**Meta:** um repositório `cosmic-taskbar`, um binário, código compartilhado, sem piorar consumo.
**Spec:** `docs/superpowers/specs/2026-10-30-unificacao-design.md`
**Base de medição (30/10, antes da fusão):** barra 3 × ~18,8 MB PSS, 36 threads, 36 fds; botão Iniciar 3 × ~10 MB; serviço `--menu` ~34 MB; interação com a prévia: mediana ~12% de CPU, 30 quadros/s.

## Restrições globais

- IDs de applet: `io.github.Aenes0.CosmicTaskbar` e `io.github.Aenes0.CosmicStart` (o painel os referencia).
- libcosmic `5a8bd94`, cosmic-protocols `c0cff4d`, sctk 0.20.0, edition 2024.
- Renderizador por software; sem GPU na barra e no botão.
- Testes de tela real só com aviso ao usuário; reinício do painel reinicia todos os applets.
- Ações públicas (criar o repo no GitHub, push, arquivar os antigos): só com confirmação.

## Fase 0: preparar os repositórios antigos (precisa do OK do usuário)

- [ ] 0.1 Remover os diagnósticos temporários da prévia da barra (`TEMP-FLAGS`, contadores, log `TEMP preview`), rodar `cargo test` (esperado: 29 passam) e `cargo build --release`.
- [ ] 0.2 O usuário aprova commitar o que está pendente na barra (nome da janela, bordas finas, título em duas linhas, 30 quadros) e no menu (`log_iotc.txt` fica de fora). Sem isso o histórico importado perde essas mudanças.
- [ ] 0.3 Parar o antigo `--menu` ao trocar de binário (`pkill -f "cosmic-ext-applet-start --menu$"`).

## Fase 1: repositório novo com os dois históricos

- [ ] 1.1 `git subtree add --prefix=import/taskbar ../taskbar main` e `--prefix=import/start ../start-menu main` em `~/Projetos/cosmic-taskbar`. Verificar: `git log --oneline | wc -l` ≥ 7 + 9 commits.
- [ ] 1.2 Criar `Cargo.toml` único (`name = "cosmic-taskbar"`, binário `cosmic-taskbar`), união das dependências das duas bases na versão da barra; `[patch]` de sctk e cosmic-protocols como na barra. Sem o recurso `wgpu`.
- [ ] 1.3 Mover o código para `src/bar/` (da barra) e `src/start/` (do menu), `src/shared/` vazio; `src/main.rs` despacha os modos: `taskbar`, `start`, `--menu`, `--tools`, `--show-desktop`. Cada modo aplica antes de qualquer thread o seu `TOKIO_WORKER_THREADS`/`ICED_BACKEND` (menu usa 1 worker).
- [ ] 1.4 Portar o menu para o libcosmic `5a8bd94`: `cargo build --release` até compilar; registrar cada incompatibilidade corrigida.
- [ ] 1.5 Reaplicar os 4 patches do `iced_tiny_skia` sobre `5a8bd94` (`git apply --check` primeiro); se algum não aplicar, relatar e seguir sem ele, medindo a diferença.
- [ ] 1.6 Corrigir a visibilidade de `entry` (`load.rs:358-361`) e fazer `cargo test` compilar; meta: todos os testes das duas bases passam (barra 29 + menu 424).
- [ ] 1.7 `install.sh` e `setup.sh` únicos; dois `.desktop` (`Exec=@bindir@/cosmic-taskbar taskbar|start`). Verificação em sandbox: `desktop-file-validate`.

**Ponto de entrega 1:** instala e funciona igual às duas bases. Medir com `scripts/measure.sh` e comparar com a base acima.

## Fase 2: compartilhar o que é igual

Cada item: testes primeiro, depois trocar o uso nas duas pontas, depois apagar a cópia.

- [ ] 2.1 `shared::anim`: um único módulo de molas (`Spring`, `settled`, `settled_loose`, `motion_scale`, deslocamento do menu), unindo `bar/anim.rs` e `start/anim.rs`.
- [ ] 2.2 `shared::texts`: um módulo de i18n pt/en com as duas structs.
- [ ] 2.3 `shared::capture`: a captura do menu (`capture.rs` + worker + `shrink`) vira a única; a barra a usa no lugar de `wayland_handler` (captura, `capture_wait`). Verificação: prévia da barra com o mesmo tempo da primeira miniatura (hoje ~30 ms) e nenhuma captura presa.
- [ ] 2.4 `shared::toplevels`: uma assinatura de janelas para os dois (cctk), substituindo `bar/wayland_handler` e `start/toplevels`.
- [ ] 2.5 `shared::icons`: o cache e a pré-rasterização do menu servem à barra (ícones da lista e das prévias).
- [ ] 2.6 `shared::style`: tokens comuns (raio, vidro, sombra, alfa) usados pelo cartão de prévia e pelo menu.
- [ ] 2.7 `shared::preview`: um cartão de prévia (o da barra, com título em duas linhas, retrato e 30 quadros) usado pela barra e pelo menu "Abertos".

**Ponto de entrega 2:** medir de novo; relatar PSS, threads e fds por processo antes e depois.

## Fase 3: fechar

- [ ] 3.1 README único em pt-BR (instalação, modos, atalhos) e `gaps.md` com o que sobrou.
- [ ] 3.2 Nota no Vault Brain (hub do projeto + decisão de arquitetura) e `brain-lint.py` com exit 0.
- [ ] 3.3 Com confirmação do usuário: criar `Aenes0/cosmic-taskbar` no GitHub e enviar; colocar aviso de depreciação no README dos dois repositórios antigos e arquivá-los.
- [ ] 3.4 Revisão final (`revisor` fable) do diff completo e uma passada de AgentShield.

## Review Focus (o que a fusão pode quebrar sem teste óbvio)

1. Painel inicia o applet certo: os `.desktop` dos dois IDs apontam para o modo certo; sem argumentos o binário não escolhe um modo por acidente.
2. Atalhos Super e Win+X: continuam chamando `--menu`/`--tools`; um serviço antigo ainda vivo não responde no lugar do novo.
3. Fixados e configuração: a barra continua escrevendo `favorites` em `com.system76.CosmicAppList`, e o menu continua com a própria chave.
4. Consumo: nenhuma lib de GPU carregada na barra nem no botão; threads e fds não sobem.
5. Histórico: `git log --follow` num arquivo movido ainda mostra os commits antigos.
