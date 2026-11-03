# Medições de baseline — hub de dados único

Data: 2026-10-30. Desktop em uso (3 saídas: DP-3, DP-1, HDMI-A-1). Versão instalada já com o `--panels`
(unit `central-panels`). Janela de 60 s, painéis fechados, sem interação.
Fonte: `/proc/PID/status` (VmRSS, Threads, `*ctxt_switches` somados em todas as threads),
`/proc/PID/smaps_rollup` (PSS) e `/proc/PID/stat` (utime+stime).

## Por processo

| PID | tipo | saída | RSS MB | PSS MB | threads | trocas/60 s | wakeups/s | CPU ms/60 s |
|---|---|---|---|---|---|---|---|---|
| 454082 | monitor | HDMI-A-1 | 54,6 | 28,7 | 6 | 642 | 10,7 | 40 |
| 454089 | monitor | DP-1 | 54,8 | 28,8 | 6 | 604 | 10,1 | 30 |
| 454157 | monitor | DP-3 | 54,9 | 28,8 | 6 | 600 | 10,0 | 30 |
| 454084 | central | HDMI-A-1 | 31,0 | 8,3 | 14 | 146 | 2,4 | 80 |
| 454091 | central | DP-1 | 30,8 | 8,3 | 14 | 160 | 2,7 | 90 |
| 454322 | central | DP-3 | 30,7 | 8,3 | 14 | 172 | 2,9 | 90 |
| 454087/95/460 | tray | ×3 | 27,7 | 7,0 | 6 | 0 | 0 | 0 |
| 454083/90/213 | show-desktop | ×3 | 27,0 | 6,6 | 6 | 0 | 0 | 0 |
| 444887 | panels | — | 61,8 | 35,3 | 19 | 30 | 0,5 | 0 |
| 444847 | clipboard | — | 14,1 | 2,3 | 2 | 0 | 0 | 0 |
| 444871 | notifications | — | 13,4 | 2,2 | 1 | 0 | 0 | 0 |

## Por tipo (soma)

| tipo | n | RSS MB | PSS MB | threads | trocas/60 s | CPU ms/60 s |
|---|---|---|---|---|---|---|
| monitor | 3 | 164,4 | 86,3 | 18 | 1846 | 100 |
| central | 3 | 92,5 | 24,9 | 42 | 478 | 260 |
| tray | 3 | 83,2 | 21,1 | 18 | 0 | 0 |
| show-desktop | 3 | 80,9 | 19,9 | 18 | 0 | 0 |
| **12 applets** | 12 | **421** | **152** | 96 | 2324 | 360 |
| + panels, clipboard, notifications | 3 | 89 | 40 | 22 | 30 | 0 |

RSS conta bibliotecas compartilhadas em cada processo: a soma real de memória é o PSS (152 MB nos 12
applets). O piso de um applet libcosmic que quase nada faz (ShowDesktop) é **27 MB RSS / 6,6 MB PSS**.

## De onde vêm os wakeups (30 s, por thread)
- Monitor: `tokio-rt-worker` ×2 (86 e 84), thread principal ×2 (48 e 47), `smithay-clipboard` 45. Isso
  dá cerca de 50 trocas por Tick de 5 s: a leitura (thread de vida curta), o redesenho da barra e o thread
  do clipboard do smithay, que acorda a cada quadro.
- Central: `tokio-rt-worker` 32 e 24, principal 19 e 7, `smithay-clipboard` 12. Nenhum sinal D-Bus de
  sessão relevante em 60 s; 5 `PropertiesChanged` no barramento de sistema; 1 evento do PulseAudio em
  20 s. A causa de cerca de 2,6 wakeups/s não foi atribuída por subscription sem instrumentar (fase 0 do
  plano). Suspeitos: timers da privacidade (30 s), relógio (1/min) e os runtimes tokio de 2 threads.
- Processos auxiliares: 4 `pactl subscribe` (filhos dos applets), um por Central mais um.

## Subscriptions na barra (com o painel fechado)
Central ×3, cada uma com conexão própria:
- `services::subscription`: NM, BlueZ, UPower (bateria e periféricos) e power-profiles no barramento de
  sistema; brilho no settings-daemon da sessão; volume via `pactl subscribe`.
- cosmic-config (DnD, tema, hot edge e outros), workspaces (Wayland), a11y (Wayland) e leitor de tela.
- MPRIS (só o painel usa), privacidade (`pactl` mais timer de 30 s), notificações (sinal) e clipboard
  (sinal; só o painel usa).
- Relógio (1/min), `CheckDoNotDisturb` (20 s, se houver prazo) e clique.

Monitor ×3, cada um com:
- Tick de 5 s com `sample::reading(full=false)`: /proc/{stat,meminfo,net/dev,loadavg,uptime}, RAPL,
  hwmon da CPU e NVML (uso, temp, clock, watts).
- Tempo: stat de arquivo a cada 60 s.
- Contador de energia da GPU (cerca de 10 ms) a cada 60 s, só no applet que tem o lock.

Tray ×3: watcher SNI, mais `NewIcon` de cada item e cópia própria dos ícones. ShowDesktop ×3: thread
Wayland parada em eventos.

## Não medido
- Custo por subscription isolada: exige contadores por mensagem (fase 0 do plano).
- RSS do `cosmic-panel` atribuível aos 12 clientes: 117 MB no processo do painel, sem divisão.
