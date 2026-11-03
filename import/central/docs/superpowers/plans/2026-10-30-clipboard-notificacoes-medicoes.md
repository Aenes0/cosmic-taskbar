# Clipboard, notificações e travadas do desktop — medições (2026-10-30)

Método: o mesmo que achou o DDC.
- Amostragem a cada 2 ms do estado de todas as threads do cosmic-comp (`/proc/1558/task/*/stat`); conta os
  trechos em D (sono não interrompível) e o maior deles.
- Espera da thread principal na runqueue (`schedstat`).
- CPU e trocas de contexto do serviço em teste.

Em repouso: 0 trechos em D ≥ 20 ms em todas as janelas (5–20 s).

**Descartadas**: as rodadas das 01:36–01:38, por contaminação com prints e notificações do próprio usuário. São
elas: "toggle do Central × menu Iniciar × nvidia-smi" e a primeira tentativa de "Central abrir/fechar".
O resultado do Central foi refeito depois, sozinho, com bisseção.

## A — serviço de notificações (`become_monitor`)
- Regras de match (`notifications/watch.rs` l.83–96): só chamadas `Notify`, respostas vindas de
  `org.freedesktop.Notifications`, sinais da interface de notificações e `NameOwnerChanged` desse nome. Não
  recebe o tráfego geral do barramento.

| estímulo | maior D comp | serviço CPU / trocas | fila comp |
|---|---|---|---|
| 5 notify-send | 0 ms (1 amostra) | 0 ms / 31 | 9,6 ms |
| 50 notify-send em sequência | 0 ms (1 amostra) | 120 ms / 352 | 17,5 ms |
| 400 chamadas D-Bus de outro app | 0 | 0 / 0 | 29,4 ms |

- Sem evidência de travada. O canal "TrySendError Full" de 28/10 não pôde ser visto: o journal desse boot não
  existe mais (boots retidos: 21/10 e o atual).

## B — abrir o Central com histórico grande
Host de teste com `XDG_STATE_HOME` próprio, 4 aberturas cada:

| estado | mostrado (ms) | maior D comp por abertura (ms) |
|---|---|---|
| vazio | 110, 17, 3, 18 | 213, 91, 20, 256 |
| 44 notificações | 69, 5, 5, 6 | 176, 121, 17, 163 |
| clipboard com 4×1 MB de texto e 4 PNG de 9,6 MB sem miniatura | 97, 45, 45, 46 | 150, 124, 5, 52 |

- O histórico **não** muda as travadas: com tudo vazio elas já aparecem.
- O clipboard grande só encarece o próprio painel (mostrado +40 ms, RSS do host +5 MB).

## C — serviço de clipboard

| estímulo | maior D comp | serviço CPU / trocas | fila comp |
|---|---|---|---|
| texto 64 KB | 0 (1 amostra) | 10 ms / 5 | 9,0 ms |
| texto 1 MB | 0 | 0 / 9 | 3,0 ms |
| PNG de 9,6 MB | 0 | 50 ms / 13 | 26,2 ms |
| 20 cópias seguidas | 0 (1 amostra) | 20 ms / 163 | 17,8 ms |

- Sem evidência de travada.
- A leitura já ocorre numa thread à parte (`clipboard/service.rs` l.551).

## A causa real: o que o Central assina ao abrir
Bisseção (cópia do código em `target/`, sem mexer no produto): só uma subscription ligada por vez, abrindo e
fechando o Central a cada 1 s, por 12 s.

| subscription ligada | trechos D ≥ 20 ms | maiores (ms) |
|---|---|---|
| nenhuma | 0 | — |
| todas | 28 | 120, 121, 126 |
| `services::subscription` | 57 | 144, 156, 202 |
| `services::a11y_subscription` (Wayland) | 14 | 92, 155, 167 |
| settings, workspaces, leitor de tela, mídia, privacidade, notificações, clipboard, tarefas de abertura | 0 | — |

Dentro de `services::subscription`:

| watch ligado | trechos D ≥ 20 ms | maiores (ms) |
|---|---|---|
| **brilho** (`com.system76.CosmicSettingsDaemon`) | 23 | 124, 124, 130 |
| rede, Bluetooth, bateria, periféricos, power-profiles, volume | 0 | — |

Comandos isolados testados a 1/s:

| comando | trechos D ≥ 20 ms |
|---|---|
| `cosmic-randr list`, `systemctl is-active`, `pactl` (sinks, sources, source-outputs, subscribe), `wpctl status`, `upower -d`, varrer `/proc/*/fd` | 0 |
| `pw-dump` (só rodaria com a câmera aberta via PipeWire) | 9, até 126 ms |

## D — comparação
- DDC por `ddcutil`: até 235 ms por leitura, em todas as saídas.
- Assinar o brilho do cosmic-settings-daemon: até 130 ms. Provavelmente o daemon lê o brilho dos monitores
  externos por DDC/i²c; não verificado no código do daemon.
- Proxy de a11y (Wayland): até 167 ms.
- Notificações e clipboard: 0 (nenhum trecho ≥ 20 ms em nenhum estímulo).
- **O usuário tem razão em parte: o `ddcutil` do próprio Central já foi removido da abertura, mas a
  subscription de brilho e a de a11y continuam travando o compositor a cada abertura e fechamento.**

## Efeitos colaterais dos testes (atenção)
- O histórico é limitado. As cópias e notificações de teste empurraram para fora itens antigos não fixados:
  cerca de 14 entradas do clipboard (com as imagens) e cerca de 55 notificações antigas.
- Os itens de teste foram removidos depois.
- Cópia anterior intacta no snapshot do snapper `/home/.snapshots/80/snapshot/alexandre/.local/state/io.github.Aenes0.CosmicCentral/`:
  - `clipboard.json` de 23:38 e a pasta `clipboard/`;
  - `notifications.json` de 00:53.
- Restaurar exige parar `central-clipboard` e `central-notifications`, copiar os arquivos e iniciar de novo. Não
  foi feito: decisão do usuário.
- O clipboard do usuário foi salvo e restaurado (texto). Depois disso o usuário copiou uma imagem.
