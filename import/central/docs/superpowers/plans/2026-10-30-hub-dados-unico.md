# Plano — hub de dados único (`--hub`) e applets finos

Spec: `docs/superpowers/specs/2026-10-30-hub-dados-unico-design.md`. Baseline:
`docs/superpowers/plans/2026-10-30-hub-medicoes.md`. Direto em `main`, sem commit, sem instalar e sem
reiniciar o painel sem avisar. Medições no host via `flatpak-spawn --host`. As medições de cada fase usam
o mesmo script do baseline: 60 s, painéis fechados, por tipo de processo.

## Fase 0 — Instrumentar (sem mudar comportamento)
- Contador por Message/subscription atrás de `COSMIC_HUB_TRACE=1`: quantas mensagens por fonte em 60 s, em
  Central e Monitor.
- Teste: o contador soma e zera (lógica pura).
- Saída: tabela "subscription → mensagens/min → CPU" anexada às medições. Explica os 2,6 wakeups/s do
  Central e os 10/s do Monitor.

## Fase 1 — Esqueleto do hub
- `src/hub/{mod,snapshot,service}.rs`:
  - snapshot versionado (`version = 1`), seções com serde;
  - D-Bus com `Snapshot(as)`, `Generation` e `Changed(t, as)`;
  - `Hold`/liberação via `NameOwnerChanged`;
  - watchdog (reaproveita `panels/watchdog.rs`).
- `main.rs`: modo `--hub`. `data/central-hub.service.in` e o `.service` de ativação, com os limites da
  spec; `install.sh` preparado, sem rodar.
- Testes:
  - serde nos dois sentidos de cada seção;
  - versão desconhecida é rejeitada sem pânico;
  - `Changed` lista só as seções que mudaram (diff puro);
  - `systemd-analyze --user verify` passa.
- Saída: `busctl --user call … Snapshot as 0` responde; hub ocioso com 0 trocas/60 s fora do batimento;
  menos de 10 MB de PSS.

## Fase 2 — Dados do Central no hub
- Mover para o hub:
  - `services::subscription`: NM, BlueZ, UPower, power-profiles, brilho e volume;
  - privacidade e contagem de notificações;
  - cosmic-config da barra (DnD, tema, hot edge, show_percentage, panel_icons, bar_spacing).
- Cliente no applet Central (`src/hub/client.rs`):
  - assina `Changed`, pede as seções e aplica as mesmas `Message::Update` de hoje, para o view não mudar;
  - mantém a regra de 2 s do ajuste.
  - Ligado por `COSMIC_CENTRAL_HUB=1` para comparar A/B; depois vira padrão.
- Ações `StepVolume`, `ToggleMute`, `StepBrightness` e `SetShowPercentage` no hub.
- Testes: aplicar snapshot → mesmo estado do App que as mensagens diretas (puro); latência do scroll medida
  (menos de 30 ms).
- Saída: capturas da barra idênticas A/B nas 3 saídas; Central abaixo de 1 wakeup/s.

## Fase 3 — Monitor no hub
- A amostragem da barra (5 s, `SCHED_IDLE`, timeout), o anel de histórico, o tempo (`FetchLock`) e o
  `Meter` (flock) vão para o hub. `Hold("monitor-fast")` dá 1 s com o painel aberto.
- O applet Monitor só lê a seção `monitor` e mantém modos, limites e "linger" (puro, já tem testes).
- Testes:
  - energia com applet antigo e hub juntos: só um conta (flock);
  - ledger igual após 10 min contra a soma de RAPL;
  - linger e Over com leituras sintéticas.
- Saída: Monitor de 28,8 para no máximo 8 MB de PSS cada; menos de 1 wakeup/s cada; energia sem lacuna nem
  dupla contagem.

## Fase 4 — `--panels` lê o hub
- Ao abrir, o host aplica o snapshot do hub antes do primeiro quadro.
- Deixam de rodar no host: `services::subscription`, privacidade e contagem de notificações (vêm do hub).
- Continuam só com o painel aberto: MPRIS, clipboard, listas de Wi-Fi/sinks/mixer, DDC e processos.
- O Monitor do painel desenha o histórico do hub: os gráficos já vêm cheios.
- Testes: abrir com hub fora do ar → painel abre com o último estado e sem erro.
- Saída: "pedido → primeiro quadro" igual ou melhor que o da medição anterior; dados presentes no primeiro
  quadro (captura).

## Fase 5 — Afinar applets
- Barra Central e Monitor: remover as subscriptions movidas, `TOKIO_WORKER_THREADS=1` e nenhum código de
  imagem no caminho da barra.
- Tray e ShowDesktop: só `TOKIO_WORKER_THREADS=1`, se medir ganho.
- Saída, contra o baseline:
  - 12 applets mais o hub: no máximo 120 MB de PSS (eram 152);
  - wakeups no máximo 2/s no total (eram cerca de 38);
  - CPU no máximo 150 ms/min (eram 360).

## Fase 6 — Instalação e verificação (AVISAR antes: reinicia o painel)
- `install.sh` com `central-hub` e as units atualizadas; recarregar os applets uma vez.
- Verificar:
  - repouso de 60 s;
  - `kill -STOP` no hub: ícones param de atualizar, barra e cursor seguem;
  - watchdog reinicia o hub;
  - todas as funções da lista de riscos da spec, à mão.
- Reverter: os applets antigos caem para as próprias subscriptions se o hub não existir por mais de 30 s
  (guarda temporária, removida depois de uma semana estável).
