# Hub de dados único para os applets da barra

Estado: proposta. Aguarda as decisões do usuário (fim deste documento). Execução direto em `main`, sem commit.
Baseline: `docs/superpowers/plans/2026-10-30-hub-medicoes.md`.

## Problema
São 12 applets na barra: Central, Monitor, Tray e ShowDesktop, um de cada por monitor. Somam 421 MB de RSS
e 152 MB de PSS. Central e Monitor assinam e amostram tudo três vezes:
- cada Monitor lê /proc, RAPL, hwmon e NVML a cada 5 s, com cerca de 10 wakeups/s;
- cada Central tem as próprias conexões ao NM, BlueZ, UPower, power-profiles, settings-daemon e
  `pactl subscribe`.

O host `--panels` repete as subscriptions quando abre, então os dados chegam frios.

## Objetivo
1. Um **hub** (`--hub`) assina os serviços e amostra o sistema **uma vez**:
   - NM, BlueZ, UPower (bateria e periféricos), power-profiles, brilho, volume, privacidade e a contagem
     de notificações novas;
   - a amostragem da barra do Monitor, o histórico dele, o tempo e o medidor de energia.
2. Applets **finos** por monitor só desenham a barra a partir do snapshot do hub:
   - sem subscriptions próprias pesadas, sem imagens e com 1 thread tokio;
   - ações (scroll, mute, clique do meio) passam por métodos do hub.
3. O `--panels` lê o mesmo hub: abre com dados quentes e com o histórico do Monitor já cheio, o que corrige
   a regressão "gráficos vazios a cada abertura". O que é só do painel continua nele, só com o painel
   aberto: listas de Wi-Fi, sinks, mixer, MPRIS, clipboard, DDC e processos.

## Não-objetivos
- Mudar o visual da barra ou dos painéis.
- Mexer no cosmic-panel.
- Fundir os 4 modos num processo (ver "Fusão").
- Deduplicar o Tray nesta etapa.

## Onde vive o hub: serviço novo `--hub`
- **Unit própria** `central-hub.service`, com ativação D-Bus como `--clipboard` e `--notifications`.
- Por que não no `--panels`:
  - o watchdog do `--panels` o reinicia se a UI trava, e o hub não pode cair junto;
  - o `--panels` carrega iced, renderizador e 35 MB de PSS, e o hub não precisa de nada disso.
- Por que não no `--clipboard` nem no `--notifications`: são domínios de falha distintos, com dados do
  usuário. Misturar faria um bug de amostragem derrubar o histórico.
- Unit:
  - Type=notify, WatchdogSec=20 com batimento do loop principal, Restart=on-failure;
  - Nice=5, CPUWeight=50, IOWeight=50, MemoryMax=80M, TasksMax=32, OOMScoreAdjust=300.
  - Fica um degrau acima do `--panels` porque os ícones dependem dele, e abaixo do compositor.
- Sem Wayland e sem iced: tokio de 1 thread, mais threads `SCHED_IDLE` de vida curta para a amostragem
  (já existe `slow::blocking`).

## Protocolo hub → clientes (D-Bus, sessão)
- Nome `io.github.Aenes0.CosmicCentral.Hub`, caminho `/io/github/Aenes0/CosmicCentral/Hub`.
- Dados:
  - propriedade `Generation: t`;
  - método `Snapshot(sections: as) -> (version: u, generation: t, json: s)`;
  - sinal `Changed(generation: t, sections: as)`.
- O snapshot é serde em JSON (serde_json já é dependência), versionado (`version = 1`) e pequeno: menos de
  2 KB para a barra. Seções:
  - Central: `net`, `bt`, `power`, `battery`, `peripherals`, `volume`, `brightness`, `privacy`,
    `notifications` (só a contagem de novas), `settings`;
  - Monitor: `monitor` (a leitura da barra), `monitor_history` (anel), `weather`, `energy`.
- O cliente recebe `Changed` e pede só as seções que usa. Versão diferente da sua: registra no log e mantém
  o último estado. O cliente nunca bloqueia: chamadas assíncronas com timeout de 1 s, fora da UI.
- Ações:
  - `StepVolume(i)`, `ToggleMute()`, `StepBrightness(i)` e `SetShowPercentage(b)`;
  - os toggles dos tiles da barra seguem o caminho atual, as funções de `services`, mas passam a ser
    chamados pelo hub.
- O ajuste em curso não pode puxar o slider para trás. Hoje isso é a regra de 2 s ignorando `Update`
  após o usuário mexer; ela continua no cliente.
- Amostragem do Monitor: 5 s por padrão; 1 s enquanto um cliente segura `Hold("monitor-fast")`, liberado ao
  sair ou se o cliente some do barramento (o hub observa `NameOwnerChanged`).
- **Hub morre ou trava**: os clientes mantêm os últimos valores (os ícones só ficam velhos). Ao ver o nome
  voltar (`NameOwnerChanged`), pedem o snapshot completo. A ativação D-Bus e a unit reiniciam o hub.
  Nada no applet espera o hub.
- **Energia**: o `Meter` passa ao hub e mantém o `flock` em `energy.lock`. Durante a migração, um applet
  antigo ainda vivo continua contando e o hub só lê; depois só o hub conta. O ledger, o `energy.reset` e o
  save atômico ficam iguais.
- **Tempo**: o hub busca, guarda o arquivo e mantém o `FetchLock`. Clientes só recebem a seção `weather`.

## O que a barra precisa (mapa)
- **Central**:
  - privacidade (câmera/mic);
  - sino com a contagem de novas;
  - ícones de tiles na barra: VPN, BT, energia, DnD, tema, avião, mute, keep-awake, tiling, lupa e
    inversão, leitor de tela, alto contraste, hot edge, câmera Iriun, touchpad;
  - rede, volume (scroll, clique do meio, clique direito), periféricos (% e nome), bateria (%, carga,
    tempo, scroll de brilho) e relógio (1/min, local).
  - Mídia não aparece na barra.
  - Tooltips: não há (o `tip` descarta o texto).
- **Monitor**: tempo, CPU (uso e temp), GPU (uso e temp), memória, potência (RAPL, GPU e restante) e rede
  (down/up), com os modos Never/Always/Over e o "linger" de 10 s. O histórico não aparece na barra.
- **Tray**: SNI. **ShowDesktop**: toplevels via Wayland. Os dois ficam fora do hub.
- Estado que precisa de Wayland (tiling, lupa, inversão): o hub não tem Wayland. Esses tiles continuam no
  applet, assinados só se estiverem na barra (decisão do usuário, item 4).

## Fusão dos 4 modos num applet por monitor: inviável como pedido
- O cosmic-panel cria um processo por entrada de `plugins_*` por saída (`spawn_clients`,
  `space/wrapper_space.rs` l.373–807, socket Wayland próprio por cliente).
- Todas as toplevels de um cliente recebem o mesmo índice e o mesmo lado (`space/layout.rs` l.153–191).
- Subsurfaces ficam presas à toplevel-mãe (`space/panel_space.rs` l.1879–1990, "TODO: support input in
  subsurfaces").
- Conclusão:
  - Monitor (esquerda) com o resto (direita): impossível.
  - Tray, Central e ShowDesktop (vizinhos à direita): possível só como um widget composto, perdendo
    overflow/prioridade individuais e o isolamento de falha. Continua um processo por monitor (o painel
    não compartilha clientes entre saídas).
- Não recomendado. O ganho vem do hub, não da fusão.

## Tray e ShowDesktop
- ShowDesktop: 0 wakeups e 6,6 MB de PSS. Só `TOKIO_WORKER_THREADS=1`, se ainda não estiver.
- Tray: 0 wakeups com o painel fechado. Deduplicar exigiria um hub de SNI repassando pixmaps (complexo,
  risco de ícone errado). Fica para depois; nesta etapa só se afina.

## Metas e expectativa honesta
- O alvo de 15–20 MB de **RSS** por applet é improvável: o piso medido de um applet libcosmic quase vazio
  é 27 MB de RSS (6,6 MB de PSS), porque RSS conta bibliotecas compartilhadas.
- Metas propostas, em PSS e wakeups:
  - Monitor de 28,8 para no máximo 8 MB de PSS;
  - Central de 8,3 para no máximo 7 MB;
  - wakeups da barra de cerca de 38/s para no máximo 2/s no total, com o hub incluído;
  - CPU dos 12 applets mais o hub de 360 para no máximo 150 ms/min;
  - abrir o painel com dados presentes no primeiro quadro.

## Riscos
- **Regressão visual por dado ausente ou velho**: primeiro snapshot atrasado, seção nova faltando. Os
  clientes mostram o último estado; testes de serde nos dois sentidos; comparação de capturas
  antes×depois.
- **Semântica de notificações** ("novas", "vistas"): o hub só conta; marcar como vista continua no painel
  via serviço de notificações.
- **Latência do scroll de volume/brilho** passando pelo hub: medir menos de 30 ms; se não der, o applet
  chama o serviço direto (mantém o caminho atual).
- **Energia**: dupla contagem ou lacuna na migração. O `flock` decide; teste com applet antigo e hub juntos.
- **Hub como ponto único**: travado, os ícones ficam velhos. Watchdog de 20 s; clientes nunca bloqueiam.
- **Tráfego**: 3 a 4 clientes por `Changed`, menos de 2 KB. Desprezível frente ao custo atual.
- **Funções que não podem sumir**: clique direito (OpenAt → `--panels`), scroll de volume e brilho,
  clique do meio, % de periféricos, privacidade, relógio, modos Over do Monitor, prévias
  `COSMIC_*_PREVIEW`.

## Decisões do usuário
1. Meta de memória em PSS (proposta acima) em vez de 15–20 MB de RSS.
2. Hub como serviço novo `--hub` com unit própria (recomendado) ou dentro de outro processo.
3. Tray fora do hub nesta etapa.
4. Tiles que dependem de Wayland (tiling, lupa, inversão): ficam no applet e só assinam se estiverem na
   barra (recomendado), ou o hub abre uma conexão Wayland.
5. Ações pela barra via hub (recomendado) ou direto pelo applet.
