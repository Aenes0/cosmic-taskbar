# cosmic-cosmetics — design

Data: 2026-10-22 (revisão 2026-10-23: alt-tab com encaixe e patch do comp, §4/§7/§9/§11) · Estado: aprovado · Autor: Alexandre (decisões) / Claude (redação)

> Convenção: **[decidido]** = dito pelo Alexandre. **[minha escolha]** = escolha do redator, a conferir.
> **[agente]** = relatado por subagente de leitura, não verificado pelo redator; o builder deve confirmar no código antes de depender disso.

## 1. Intenção

Hoje a experiência do Alexandre no COSMIC é feita de vários projetos pequenos, cada um com processo,
renderização e configuração próprios: quick-settings/Central (~26k LOC, libcosmic), notificacoes (~12k,
motor leve), cosmic-workspaces (alt-tab), taskbar (~4k), start-menu (~15k), logi-osd (~5k), clipboard,
bandeja, wallpapers-monitores, iriun-adb e o fork do cosmic-greeter. Os problemas:

- processos repetidos e visual duplicado (cada um reimplementa tema, ícones, sombras, texto);
- reiniciar qualquer um (principalmente o painel) causa travadinhas e lag na interface inteira;
- não existe um lugar único para ligar/desligar as peças nem para reinstalar tudo numa máquina nova.

`cosmic-cosmetics` unifica tudo num **único daemon de sessão isolado**, com um **motor de renderização
leve compartilhado**, um **painel central de configurações** e um **instalador** que repõe o mod sobre
um COSMIC limpo. O pré-requisito do usuário final é apenas ter o COSMIC instalado. [decidido]

## 2. Metas, não-metas, anti-metas

### Metas [decidido]
1. Memória total do conjunto **menor que a soma** dos processos atuais (medida antes/depois; número-alvo definido na fase 0).
2. **Zero travadinhas** no compositor ao reiniciar o daemon ou recarregar/ligar/desligar um módulo.
3. **CPU em idle ≈ 0 %**: tudo orientado a eventos (Wayland, D-Bus, inotify, timers únicos), sem polling.
4. **Início de sessão rápido**: barra e notificações aparecem junto com o desktop.
5. Paridade verificada com cada projeto atual antes da troca (ver §10).
6. Painel central que liga/desliga cada módulo; instalador que reaplica tudo sobre um COSMIC limpo.

### Não-metas
- Compositor próprio. O contrato com o compositor fica documentado para o futuro, mas não é construído.
- Rodar fora do COSMIC (outros compositores).
- Distribuição pública (AUR etc.): **o público é só as máquinas do Alexandre por enquanto** [decidido]. Sem detecção de hardware genérica, instalação multiusuário nem testes em hardware variado na v1; a localização do clima é configurável. Pode abrir depois.
- Congelamento: os projetos atuais ficam **congelados para funções novas** durante a construção, só correções de bug; cada módulo tem ponto de corte na tabela de paridade. [minha escolha]
- Janela de configurações dentro do daemon (ela é processo separado, §6).
- Absorver `super-economia` (precisa root/polkit) e `waydroid-setup` (instalador único). [agente: relevância] O módulo de energia só a chama via pkexec/D-Bus como hoje.

### Anti-metas (fracasso mesmo que "funcione") [decidido]
- Ficar mais pesado que os projetos separados.
- Quebrar a cada atualização do COSMIC.
- Um bug num módulo derrubar a barra inteira.
- Perder qualquer recurso que hoje funciona.

## 3. Arquitetura

```
cosmic-cosmetics (workspace Cargo)
├── crates/engine        motor leve: superfícies layer-shell/popup, texto, formas, ícones/SVG, canvas, animação, tema, entrada (teclado/IME/ponteiro), a11y
├── crates/core          trait Module, supervisor, config, bus D-Bus interno, estado persistente, métricas
├── crates/modules/*     notifications, history, alttab, quicksettings (applets/tiles), clipboard, taskbar, startmenu, tray(SNI), monitor (+clima), logi, wallpapers, camera(iriun, opcional)
├── bin/cosmic-cosmeticsd        o daemon de sessão
├── bin/cosmic-cosmetics-settings  janela de configurações (sob demanda)
├── bin/cosmic-cosmetics-greeter   greeter/locker (binário de entrada próprio)
├── bin/cosmic-cosmetics-greeter-daemon  daemon root do greeter (mantido como no fork)
└── installer/           instalador, units systemd, regras udev/polkit, fallback
```

### 3.1 Processo e isolamento [decidido]
- Daemon próprio, **fora do cosmic-panel**, iniciado por `systemd --user`. Ele **substitui a barra inteira** (taskbar, applets, bandeja, quick settings, lançador).
- **Renderização em thread própria**, separada da lógica; nada do daemon bloqueia o compositor. Frames independentes.
- **Módulos isolados:** cada módulo roda em tarefa/thread supervisionada. Se um paniquear ou travar, o supervisor reinicia **só ele**, sem reiniciar o processo (panic capturado; watchdog por heartbeat).
- **Recarga a quente:** ligar/desligar módulo e mudar tema/config não reinicia o processo.
- **cgroup dedicado:** unit com limites de CPU/memória e prioridade (`Nice`, `CPUWeight`/`IOWeight` baixos) para nunca competir com o compositor. [minha escolha: valores iniciais a medir na fase 0]
- Reiniciar o daemon inteiro é um evento raro e **não pode** causar lag: o estado é restaurado do disco e as superfícies são recriadas sem tocar nas janelas dos apps.

### 3.1.1 Modelo de processos [decidido]
Isolar só por thread/panic não cobre thread presa, segfault em FFI (libdbus, hidraw, PAM), estouro de memória nem erro de protocolo Wayland. Quando o processo morre, a exclusive zone da barra some e o compositor reorganiza as janelas, que é justamente o lag a evitar. Por isso:

1. **`cosmic-cosmetics-holder`** — processo minúsculo, quase sem lógica, que mantém a superfície e a exclusive zone da barra (e as de dock/OSD que precisarem sobreviver). Não faz FFI de risco, não carrega módulos. Meta: RSS de poucos MB; nunca reinicia por causa de módulo.
2. **`cosmic-cosmeticsd`** — daemon principal com os módulos de UI. Pode reiniciar sem reorganizar janelas, pois o holder segura a zona. O desenho do que o holder expõe (buffer compartilhado, `wl_surface` entregue ao daemon, ou o holder só reserva a zona) é decisão do builder, desde que o reinício do daemon não altere a zona exclusiva.
3. **Subprocessos para módulos arriscados:** Logi (hidraw/HID++), PAM (locker/greeter) e Iriun (já é unit própria). Falha neles não derruba o daemon. Outros módulos entram em subprocesso se a medição da fase 0 mostrar crash, travamento ou vazamento; o trait `Module` deve permitir hospedagem dentro ou fora do processo sem mudar o módulo.
4. Supervisão em dois níveis: systemd supervisiona holder, daemon e subprocessos; o daemon supervisiona threads de módulo (panic capturado, heartbeat).
5. O fallback (§7) é acionado pelo estado do holder + daemon, não só pelo daemon.
6. Pergunta de memória: o holder e os subprocessos entram na soma da meta 1 (§2).

### 3.2 Motor leve [decidido]
- Base: **cosmic-text + tiny-skia**, seguindo o `notificacoes` (sem libcosmic/iced). Sem GPU obrigatória.
- Ponto de partida a reaproveitar [agente]: `notificacoes/src/render/{shapes,text,theme,glyphs}.rs`, `wayland/surface.rs`, `timing.rs` (retângulo arredondado, pílula, sombra gaussiana por máscara, texto com wrap/reticências/emoji/RTL, glifos como path, fade com easing, escala fracionária).
- **O que o motor atual não tem e precisa ganhar** [agente: `notificacoes` só trata ponteiro; `xkbcommon` está desligado de propósito]:
  - teclado (`wl_keyboard` + xkbcommon, repeat, compose) e **IME via text-input-v3** [decidido: completo na v1];
  - **AccessKit** (a11y) desde a v1 [decidido];
  - texto editável (foco, seleção, cursor), rolagem, listas grandes, slider, toggle, segmented control;
  - ícones do tema freedesktop e SVG (resvg), canvas com path/gradiente/stroke (gráficos do monitor, cena do clima), animação (slide 48 px + fade 240 ms usados hoje no popup);
  - popups/xdg_popup e layer-shell com **exclusive zone**, âncora por lado, autohide e overflow, posicionamento de popup (hoje herdados do cosmic-panel e do libcosmic) [agente: taskbar/quick-settings];
  - drag por ponteiro (o start menu já faz sem DnD do Wayland) e DnD do Wayland para fixar apps na taskbar.
- **Tema:** segue o COSMIC **ao vivo** [decidido]: cores, raio, modo escuro/claro, alto contraste, fonte, espaçamento. O parser atual (`render/theme.rs`) relê só a cada aviso, sem inotify, e escolhe o `vN` mais alto; precisa de watcher (inotify) em `cosmic-config`. [agente]
- Decisão de implementação deixada ao builder: layout declarativo próprio mínimo vs. widgets desenhados à mão por módulo. Critério: reaproveitar visual e manter RAM baixa.

### 3.3 Módulos [decidido: crates internos + trait comum]
Trait `Module` (esboço, a refinar no plano): `id`, `start`, `stop`, `handle(event)`, declaração de superfícies que precisa, de assinaturas de eventos (Wayland/D-Bus/inotify) e de seu esquema de config. O core liga/desliga pela config, isola falhas e expõe cada módulo no painel de configurações. Iriun cam é um módulo opcional, **desligado por padrão** [decidido].

Módulos da v1 [decidido]: notificações + histórico, alt-tab, quick settings + applets, clipboard, taskbar + prévias, start menu, Logi OSD / emoji picker / USB, wallpapers por monitor, bandeja (SNI), monitor do sistema (com clima), Câmera (Iriun).

### 3.4 Janela de configurações [decidido]
Binário próprio, **sob demanda**, estilo cosmic-settings (sidebar + páginas), que fala com o daemon por **D-Bus**. Só existe em memória enquanto aberta; pode usar libcosmic completo [minha escolha: o usuário aprovou a separação, não a toolkit] sem pesar o daemon. Uma página por módulo, com liga/desliga e opções; o item de Câmera (Iriun) aparece ali e na bolha do quick settings.

## 4. Módulos: o que cada um precisa trazer

Fonte do inventário: relatórios de leitura dos projetos atuais **[agente, não verificado]**. A lista é ponto de partida da tabela de paridade (§10); o builder confirma no código.

| Módulo | Origem | Recursos a preservar | Pontos de atenção |
|---|---|---|---|
| Notificações + histórico | `notificacoes` | servidor `org.freedesktop.Notifications` + interface de applet; cartões empilhados (1–8), urgência 3 níveis, barra de tempo, ações, imagens/ícones, pílula (OSD do logi), DND (arquivo do COSMIC via inotify), modo privado com tela cheia, sons, aviso USB (udev/logind), modo `--sem-tela` | **histórico hoje é só memória (200 itens, sem imagens)**; persistir em disco é trabalho novo; importar do formato do módulo de notificações do Central (`--notifications`) |
| Alt-Tab | `cosmic-workspaces` | MRU, previews por screencopy, ciclo, ativação, mover para desktop, scroll, setas, clique; disparado por atalho `Spawn` → D-Bus | **Atualizado em 2026-10-23:** o repositório `Aenes0/cosmic-workspaces-aenes` agora tem o alt-tab **com encaixe entre monitores** (`src/switcher/{grid,hit,keys,mod,mru,state,zones}.rs`, `src/view/switcher.rs`, captura mais leve), serviço de guarda dos atalhos e planos de encaixe [verificado só por `git log`/`--stat`; conteúdo ainda não lido]. O módulo `alttab` nasce **já com encaixe** e porta `zones.rs`. O encaixe depende do **patch do cosmic-comp** (§7). Soltura do Alt (ModifiersChanged na surface Exclusive) e bug da janela presa ao ponteiro no arrasto: reconferir ao vivo antes de portar. Fallback ao atalho nativo se o módulo cair |
| Quick settings + applets | `quick-settings` (Central) | 22 tiles, 12 páginas (Main, Edit, Wifi, Vpn, Bluetooth, Power, Sound, Displays, Tiling, DND, Session, Camera), brilho por monitor, áudio por app/mic/câmera, mídia (MPRIS), calendário/relógio, espaçamento da barra, workspaces/tiling, show-desktop, edição de layout em grade | UI amarrada ao libcosmic/iced; lógica (`services.rs`, `audio.rs`, `media.rs`) independente de toolkit e reaproveitável. Gravadores de tema/modo escuro usam `cosmic_theme` |
| Clipboard | `quick-settings/clipboard` | histórico texto/imagem, fixar, recopiar; `ext_data_control_v1`; dono único de `clipboard.json`; D-Bus `...CosmicCentral.Clipboard` | hoje já é um processo separado; passa a módulo |
| Taskbar + prévias (e dock) | `taskbar` | **a taskbar nova substitui também o dock do cosmic-panel** [decidido]; fixados passam a ser da config própria e o importador lê os atuais (LIST_ID do COSMIC). Fila fixados+abertos, indicadores, prévias ao vivo (hover 400/300 ms, clique/botão do meio), menu de contexto, badges/progresso (Unity LauncherEntry), drag para reordenar/fixar, fixados compartilhados com o dock | hoje é applet do cosmic-panel; passa a ser desenhado por nós. Bug das **prévias presas**: não repetir o padrão (validar popup contra estado real, nunca confiar só em flag local) |
| Start menu | `start-menu` | busca (apps/janelas/arquivos), fixados com pastas, abertos, recomendados, todos (categoria/grade/lista), rodapé, Win+X, desinstalar app, redimensionar, Esc em cascata, D-Bus `io.github.Aenes0.CosmicStart` | maior custo do motor: `text_input` + `scrollable` + teclado + IME |
| Bandeja | fork do status-area | SNI watcher/item, dbusmenu, escolher itens visíveis | fork absorvido [decidido]; exige o nome `org.kde.StatusNotifierWatcher` |
| Monitor do sistema | `quick-settings/monitor` | painéis Energy/Cpu/Gpu/Memory/Disks/Network/Sensors/Processes/System; gráficos; **clima completo, ligado como hoje, com cena animada** [decidido] | clima busca rede (Open-Meteo, MET Norway, wttr.in) via curl; exige canvas com gradientes |
| Logi OSD / emoji / USB | `logi-osd` | HID++ (hidraw), bateria, Caps/Fn, tecla Emoji, hotplug | já é calloop single-thread, ~4,5 MB. Regra udev (`uaccess`) é instalada uma vez pelo instalador. **Não repropor bandeja para o Logi** [memória do usuário] |
| Wallpapers por monitor | `wallpapers-monitores` | recorte exato por saída, cache, focos manuais, regra de ampliação > 1,35x, reação a mudanças de config/saídas | hoje Python+Pillow+libvips (oneshot). **Reescrever o recorte em Rust, em thread de baixa prioridade, liberando memória ao terminar** [decidido] |
| Câmera (Iriun) | `iriun-adb` | item na bolha e na config: estado, celular/Iriun no PC e no celular, tela apagada, erros, Ligar/Desligar/Reiniciar, Mostrar/Esconder janela | **a lógica adb/scrcpy/PIN fica em processo separado** (unit própria): reiniciar o daemon não pode derrubar a câmera. O módulo vigia `~/.local/state/iriun-adb/status.json` e controla via systemd/D-Bus e sinais. **Nunca ler o PIN** |
| Greeter / locker | fork `cosmic-greeter` | visual estilo Windows 11, relógio/avatar/senha, animações, bloqueio (ext-session-lock), PAM, greetd | ver §5 |

## 5. Greeter e locker [decidido: absorvido, binário próprio]

- Mesmo workspace e mesmos componentes visuais do motor, mas **binário de entrada separado** (`cosmic-cosmetics-greeter`), porque o greeter roda antes da sessão, como outro usuário, sem depender do daemon. Substitui o fork (sai o `cosmic-greeter-aenes`).
- **Mantém o roteamento atual por usuário** [agente]: usuário `cosmic-greeter` → modo greeter (greetd); qualquer outro → modo locker (ext-session-lock, residente, disparado por logind `Lock`).
- **Funcionalidades:** as do fork atual **mais** seletor de usuário, seletor de sessão e acessibilidade básica (leitor de tela, lupa, alto contraste, inversão), que o fork atual cortou da UI [decidido: voltam]. Alexandre tem planos adicionais para o greeter que ainda não descreveu: **pergunta pendente** (§12).
- **Daemon root:** manter `cosmic-greeter-daemon` e `cosmic-greeter-config` como estão; trocar só o frontend. `run_as_user` (seteuid/initgroups) copiado intacto com testes (ler `/etc/shadow` deve dar EPERM). Não unificar o daemon root com o binário de UI. [agente — recomendação a validar]
- **Segurança da reescrita** [agente]:
  - senha em buffer zerado no drop; sem clonar por mensagens; sem log do segredo (hoje a conversa PAM faz `tracing::info!` dos prompts);
  - PAM: serviço validado, `authenticate` sempre seguido de `acct_mgmt`; `CancelSession` no greeter para o `pam_faillock` contar tentativas;
  - bloqueio: `unlock()` só após PAM OK, nunca no caminho de falha; tratar lock durante unlock; lockfile para re-bloquear após reinício;
  - uma surface de lock **em cada output, inclusive os que aparecem depois (hotplug)**, senão a tela fica exposta;
  - revisão de segurança (`security-reviewer`) obrigatória antes de ir para o sistema.
- Precisa de no motor: ext-session-lock-v1, uma surface por output, xkbcommon (repeat/compose), campo de texto/senha, IME, a11y, imagem de papel de parede 4K (`image` + JXL).
- IME e biometria (`pam_fprintd`) **não foram verificados** no fork atual.

## 6. Dados, configuração e persistência

- **Config própria** em diretório do projeto (formato a definir pelo builder; leitura/gravação atômica e inotify), com esquema por módulo. Configuração do COSMIC (tema, DND, painel, atalhos) é **lida** de `cosmic-config`; escritas no COSMIC só onde já são feitas hoje (tema/modo escuro, touchpad, hot edge, tiling) e documentadas.
- **Importador de uma vez** [decidido] dos formatos atuais: histórico/ configs de notificações, `clipboard.json` do Central, config do `notificacoes` (`~/.config/notificacoes/config`), config do Central (`io.github.Aenes0.CosmicCentral`), fixados da taskbar (config COSMIC), `wallpapers-monitores/focos.json`. O importador é idempotente e não apaga os originais.
- **Histórico de notificações em disco**: novo, com limite configurável e política de imagens (a definir no plano).
- Estado do daemon restaurado do disco no restart (§3.1).

## 7. Instalação, fallback e compatibilidade [decidido]

- **Instalador** que, sobre um COSMIC instalado: instala binários e units; **desativa** os componentes originais substituídos (cosmic-panel, cosmic-notifications, applets); instala regras udev (logi) e polkit; troca o greeter; troca o atalho de Alt+Tab; importa dados; é reversível (`uninstall`).
- **Fallback:** um unit de fallback (`OnFailure=` / contador de falhas) reativa `cosmic-panel`/`cosmic-notifications` originais se o daemon falhar **N vezes** em janela de tempo. **Nunca rodam os dois ao mesmo tempo.** N e janela: a definir no plano.
- **Watchdog** do systemd + restart em milissegundos.
- **Checagem de versão do COSMIC** no instalador e no start do daemon: protocolo/config incompatível → aviso e fallback automático.
- Alt+Tab: se o módulo cair, o atalho nativo (`System(WindowSwitcher)`) deve voltar a funcionar. [agente: previsto no spec do alt-tab, sem implementação vista]
- **Patches do compositor [decidido em 2026-10-23]:** existem hoje em `cosmic-workspaces-aenes` `patches/cosmic-comp/0001-set-position.patch` e dois patches de protocolo (`toplevel-management-set-position`, `client-toolkit-bind-v5`), mais PKGBUILDs `cosmic-comp-aenes` e `cosmic-workspaces-aenes` [verificado só por `--stat`]. Em vez de um fork do cosmic-comp, o `cosmic-cosmetics` **mantém esses patches no próprio repositório** (`patches/`), absorvendo-os como pediu ("todo conteúdo de forks absorvido"). O instalador aplica o patch (via PKGBUILD `cosmic-comp-aenes`) e a **checagem de versão** (acima) valida a compatibilidade a cada atualização do COSMIC; se o patch não aplicar, o encaixe se desliga e o resto funciona com o `cosmic-comp` original (o alt-tab segue sem encaixe). O patch é a única exceção ao "sem alteração do compositor"; nada além dele.
- Forks: nenhum fork do cosmic-comp inteiro (só os patches acima); fork do status-area e do greeter absorvidos (§4, §5).
- Packaging: PKGBUILD para CachyOS/Arch (o greeter atual usa x86-64-v3 e PAM `login`) [agente]. Outras distros: fora da v1.

## 8. Restrições técnicas conhecidas

- Wayland/COSMIC: usar `cosmic-protocols` / `cctk` e `smithay-client-toolkit 0.20` já fixados por `rev` nos projetos (`32283d7` / `c0cff4d`). Os projetos usam revs diferentes; o builder unifica. [agente]
- Sem libcosmic/iced no daemon e no greeter; permitido (e recomendado) na janela de configurações.
- Rust edition 2024, tokio/zbus 5 para D-Bus (o `notificacoes` usa `dbus` (libdbus) + calloop; unificar o modelo de eventos é decisão do builder, desde que atenda CPU ≈ 0 em idle).
- Dependências externas hoje usadas pelo Central e que precisam de decisão no plano (manter, trocar por API nativa ou tornar opcionais): `pactl`/`wpctl`, `cosmic-randr`, `ddcutil`, `curl`, `systemctl`, `flatpak-spawn`. [agente]
- Ambiente de desenvolvimento do usuário usa Flatpak sandbox (shell via `flatpak-spawn --host`); `git` pode não estar no PATH do sandbox [memória + agente].
- i18n: PT/EN próprios hoje no Central; manter ao menos PT/EN.

## 9. Ordem de construção e entrega

A troca é **de uma vez** [decidido], mas a construção segue uma ordem para ter feedback e evitar o "grande estouro":

0. **Fase 0 — medição e base:** repo + workspace; medir RSS/PSS/CPU/wakeups dos projetos atuais (linha de base das metas, ver perímetro em §10.2); definir alvos numéricos; esqueleto do `engine`/`core`/`holder`; **infraestrutura de teste: cosmic-comp aninhado em janela para barra/notificações/alt-tab e uma VM com COSMIC para instalador, fallback e greeter** [decidido]. Nada de trocar o painel na sessão diária antes do fallback estar testado na VM.
1. **Motor + core + tema ao vivo + supervisor.**
2. **Notificações + histórico em disco** (primeiro módulo; reaproveita o `notificacoes`).
3. **Quick settings, applets, clipboard, bandeja, monitor (+clima).**
4. **Barra + taskbar + prévias** (inclui exclusive zone/popups; é o ponto de maior risco de substituir o cosmic-panel).
5. **Alt-Tab.**
6. **Start menu.**
7. **Logi, wallpapers, Câmera (Iriun).**
8. **Greeter/locker** (revisão de segurança dedicada).
9. **Janela de configurações, instalador, fallback, importador, empacotamento.**
10. **Última etapa [revisada em 2026-10-23]: absorver o encaixe existente.** O encaixe/mover janela entre monitores arrastando a prévia do alt-tab **já foi desenvolvido** no repositório `cosmic-workspaces-aenes` (pedido `set_position(toplevel, output, zone)` em `cosmic-toplevel-management` + patch do cosmic-comp, reaproveitando `Shell::move_window` e `FloatingLayout::snap_to_corner`) [agente + `--stat`]. Esta etapa deixa de ser desenvolvimento novo e passa a ser: (a) trazer os patches para `patches/` do repositório novo; (b) integrar o módulo de zonas ao módulo `alttab` (fase 5 já porta `zones.rs`; aqui entra a parte que depende do patch); (c) PKGBUILD/instalador aplicando o patch e validando; (d) teste ao vivo do encaixe no compositor aninhado e na VM. Continua **por último**, depois do restante estar pronto [decidido].

As fases 2–8 podem ser executadas em paralelo por módulo depois da fase 1, por terem arquivos disjuntos.
Cada fase termina com build + testes verdes e itens da tabela de paridade marcados.

## 10. Critérios de aceite

1. **Paridade:** tabela módulo × recurso (§4, expandida no plano), cada linha marcada "igual / melhor / adiado com aprovação". Nada some sem aprovação do Alexandre.
2. **Metas medidas** (§2). Perímetro [minha escolha]: *antes* = cosmic-panel + applets + todos os projetos atuais em execução; *depois* = holder + daemon + greeter residente (locker) + auxiliares ativos (Iriun, recorte de wallpapers); a janela de configurações é medida à parte (só existe aberta). RSS/PSS total ≤ soma dos processos atuais; CPU idle ≈ 0 (wakeups/s medidos); início de sessão; zero queda de frame ao reiniciar/recarregar (medido com o compositor sob carga).
3. **Isolamento:** teste de falha injetada num módulo reinicia só o módulo; `kill -9` do daemon não afeta as janelas e o systemd o recoloca; falhas repetidas acionam o fallback e voltam os originais.
4. **Segurança:** greeter/locker passam por revisão de segurança dedicada; testes de PAM/unlock/hotplug de monitores.
5. **Instalação:** em VM/conta limpa com COSMIC, `install` leva ao desktop completo; `uninstall` devolve o original.
6. **Dados:** o importador migra o histórico/configs reais do Alexandre sem perdas (verificado por amostra).

## 11. Riscos principais

- **Motor leve é o maior risco de custo e prazo:** start menu, greeter (IME, senha, a11y), monitor/clima (canvas) e quick settings (popups, animações) exigem muito mais do que o `notificacoes` faz hoje.
- **Substituir o cosmic-panel** (exclusive zone, autohide, overflow, popups) não tem rede de proteção além do fallback.
- **"Troca de uma vez" + fallback** pode virar fallback permanente se a paridade não for verificada; por isso §10.1.
- **Atualizações do COSMIC** mudando protocolos ou `cosmic-config`.
- **Alt-Tab:** soltura do Alt e bug de arrasto ainda não confirmados ao vivo na época da pesquisa; o repositório `cosmic-workspaces-aenes` agora contém o trabalho (3 commits em 2026-10-23, incluindo encaixe e patch do comp). Reconferir o estado real e ao vivo antes de portar. **Patch do cosmic-comp:** rebase a cada release do COSMIC; é um risco novo, mitigado pela checagem de versão e pelo desligamento do encaixe se não aplicar (§7).
- **Greeter** é fronteira de segurança: erro aqui trava o login ou expõe a sessão.

## 11.1 Escolhas do redator a conferir [minha escolha]
- Aceite de a11y/IME na v1: navegação completa por teclado na barra, notificações, start menu e greeter; Orca lê o texto dos módulos principais; IME testado com o que o Alexandre usa hoje.
- Base de versão: o COSMIC instalado nesta máquina; `rev` de `cosmic-protocols`/`cctk` unificada nele; a checagem de versão aceita só a faixa testada.
- Congelamento dos projetos atuais para funções novas (§2).
- Perímetro de memória (§10.2).

## 12. Abertos

- **Do Alexandre (afeta o que é construído):**
  - Os "planos" para o greeter além de seletor de usuário/sessão e a11y básica: quais são?
  - Valores-alvo de memória/tempo de início (ou "o que for menor que hoje" basta?).
  - Local do repositório novo (os projetos atuais serão arquivados no fim [decidido]).
- **Do builder (decisão técnica, a registrar como divergência/escolha no plano):** modelo de eventos único (calloop vs tokio), formato da config, esquema do trait `Module`, política de imagens no histórico, dependências externas (§8), valores do cgroup, N/janela do fallback, unificação das `rev` dos protocolos.
