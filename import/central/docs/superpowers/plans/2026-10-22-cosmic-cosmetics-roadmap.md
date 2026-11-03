# cosmic-cosmetics — Roadmap de planos

**Spec:** `docs/superpowers/specs/2026-10-22-cosmic-cosmetics-design.md` (aprovada em 2026-10-22).

A spec tem 10 fases com dependências fortes do motor (`engine`) e do núcleo (`core`). Detalhar as fases 2–10 agora
seria inventar interfaces que só existem depois da fase 1. Por isso: **um plano detalhado por vez**, escrito quando a
fase anterior estiver verde. Este documento é o índice e fixa a ordem, as dependências e o que cada plano entrega.

| Plano | Fase(s) da spec | Entrega testável | Depende de | Pode rodar em paralelo com | Estado |
|---|---|---|---|---|---|
| `2026-10-22-cosmic-cosmetics-fase-0-1-base-motor.md` | 0 e 1 | workspace, linha de base medida, motor, núcleo, holder, units, daemon "hello" com módulo ligável a quente | — | — | **escrito** |
| `…-fase-2-notificacoes.md` | 2 | módulo de notificações + histórico em disco + importador; substitui `notificacoes` em paridade | 0–1 | 3 | a escrever |
| `…-fase-3-quicksettings.md` | 3 | quick settings/applets, clipboard, bandeja SNI, monitor (+clima) | 0–1 | 2 | a escrever |
| `…-fase-4-barra-taskbar.md` | 4 | barra com exclusive zone, taskbar+dock, prévias, popups | 0–1, holder | 2, 3 | a escrever |
| `…-fase-5-alttab.md` | 5 | alt-tab como módulo **já com encaixe** (porta `switcher/*` e `zones.rs` do `cosmic-workspaces-aenes`), com fallback ao atalho nativo | 0–1, 4 (previews/captura) | 6, 7 | a escrever |
| `…-fase-6-startmenu.md` | 6 | start menu (texto editável, rolagem, IME) | 0–1 | 5, 7 | a escrever |
| `…-fase-7-utilitarios.md` | 7 | Logi (subprocesso), wallpapers em Rust, Câmera Iriun | 0–1 | 5, 6 | a escrever |
| `…-fase-8-greeter.md` | 8 | greeter/locker novos, revisão de segurança dedicada | 0–1, daemon root mantido | 5–7 | a escrever |
| `…-fase-9-config-instalador.md` | 9 | janela de configurações, instalador, fallback, importador, PKGBUILD (incl. `cosmic-comp-aenes` com `patches/`), testes na VM | 2–8 | — | a escrever |
| `…-fase-10-encaixe.md` | 10 | absorver o encaixe já desenvolvido (patches do comp em `patches/`, integração do módulo de zonas, PKGBUILD, teste ao vivo) | tudo | — | **por último** (decidido); deixou de ser desenvolvimento novo em 2026-10-23 |

## Regras que valem para todos os planos
- Cada plano termina com build + testes verdes e as linhas da tabela de paridade (§4/§10 da spec) marcadas.
- Antes de escrever o plano da fase N, revisar a spec §4 da linha do módulo e conferir o código atual (os relatórios de
  agente da spec não são verificados).
- Nenhum plano comita: o trabalho fica no working tree para o Alexandre revisar (CLAUDE.md §3.1).
- Fase 0 entrega o ambiente de teste (cosmic-comp aninhado + VM) usado por todas as seguintes; nada de testar a troca
  do painel na sessão diária antes do fallback passar na VM.

## Atualização 2026-10-23
O repositório `Aenes0/cosmic-workspaces-aenes` passou a conter o alt-tab com encaixe e o patch do cosmic-comp (spec §4, §7, §9.10). Antes de escrever os planos das fases 5 e 10, ler esse repositório (commits `69cc42d`, `37a89bc`, `6a29ff9`) e seus specs/plans de encaixe.
