# cosmic-cosmetics — Expansão: substituir funções do COSMIC, perfis de hardware e integração

Data: 2026-10-27 · Estado: **aprovado** (o usuário deu aprovação prévia: "tudo que disse é aprovação imediata") · Autor: Alexandre (decisões) / Claude (redação)
Estende a spec `2026-10-22-cosmic-cosmetics-design.md`; onde conflitar, esta vale.

> Convenção: **[decidido]** = Alexandre. **[minha escolha]** = redator. **[verificado]** = conferido no código/máquina nesta data por agentes de leitura (arquivo:linha no ledger). **[PDF]** = afirmação da "Análise de otimização da interface COSMIC (CachyOS)" de 24/10, **não** verificada.

## 1. Intenção [decidido]
Deixar de só **substituir clientes** do COSMIC e passar a **substituir funções** dele: compositor (o que for seguro), sessão, painel, launcher, workspaces, OSD/notificações, papel de parede. Tudo integrado ao que já construímos (motor, núcleo, módulos), em **processos isolados, próximos da máquina**, sem lag de carregamento, com máxima economia — "roda até numa batata". E **escalável para fora desta máquina**: notebooks, iGPU, máquinas sem GPU, ARM/Raspberry.

## 2. O que a medição real mudou [verificado em 27/10, sem sudo]
- Sessão atual nesta estação (Ryzen 9 5900XT, RTX 5070, 32 GB, COSMIC 1.10.0): **52 processos de shell, RSS 2 285 MB, PSS 1 181 MB; 28 processos com contexto de GPU aberto**. O PDF estimava 1,0–1,5 GB; aqui é pior porque cada applet roda **3×** (uma por saída) e todos os `cosmic-ext-applet-*` usam wgpu.
- Maiores consumidores (PSS): `cosmic-comp` 270 MB, portal-cosmic 75, files-applet 74, bg 65, app-library 61, launcher 60, panel 60, start-menu `--menu` 50, pop-launcher+plugins ~55; applets ~21 processos somando ~250 MB.
- `cosmic-session` é um binário Rust com lista **fixa** de componentes (comp, panel, app-library, launcher, workspaces, bg, osd, idle, settings-daemon, notifications, greeter, files-applet), cada um em `.scope`; não há config para omitir. O instalador (fluxo INS) já decidiu desativar por **shims** em `/usr/local/bin`.
- `ananicy-cpp` ativo com `/etc/ananicy.d/00-default/DEs-and-WMs/cosmic.rules`: comp/panel/session/osd/`cosmic-applet-*` = `LowLatency_RT`; bg/idle/randr = `BG_CPUIO`; os nossos binários **não constam** (rodam no default).
- Nosso repo: 21 crates, ~114k linhas, ~2 900 testes, **mas o daemon ainda só registra `hello`** — nada está ligado. A integração é o gargalo real, antes de qualquer função nova.

## 3. O que a leitura do cosmic-comp mudou [verificado no checkout 742b800; **correção 27/10 (B3): o fonte da versão instalada `cosmic-comp-aenes 1.10.0-2` é a tag `epoch-1.10.0` (41497b42) + protocols c0cff4d — os patches partem dessa tag, e os arquivo:linha abaixo podem variar**]
| Afirmação do PDF | Veredito | Consequência |
|---|---|---|
| Backend pixman existe e está morto | **Parcial**: `GbmPixmanDevice` (kms/render/pixman.rs:29) é morto, mas o resto do comp (render/mod.rs, element.rs, shaders de sombra/animação, iced via `AsGlowRenderer`) é acoplado ao `GlowRenderer`. "Ligar" = trocar o tipo do `GpuManager` e perder os shaders. | Esforço **L**. **Não entra na v1 do fork** [minha escolha]. Sem GPU usa-se o caminho `software_renderer()` (EGL_MESA_device_software/llvmpipe, kms/mod.rs:293-305) e o **perfil leve** (§5) reduz o que ele precisa desenhar. |
| 200 ms fixos; sem chaves de animação/blur/sombra | **Confirmado**: 200 ms em shell/mod.rs:128, floating/mod.rs:55, tiling/mod.rs:82, cursor.rs:542. `AppearanceConfig` só tem clip_*/shadow_tiled_windows. **Não existe blur de fundo** no comp (só blur_strength do cursor). | Chaves `animations`, `shadows` (tiled e floating) e `overview_alpha` em `AppearanceConfig`: esforço **S–M**. Entra no fork. |
| SSD em CPU sem cache | **Parcial**: iced_tiny_skia com cache por escala, damage e `shape_run_cache` (utils/iced/mod.rs:1103-1193); `Renderer::new` é por elemento criado, não por quadro. | Nada a fazer. |
| Alt-tab é o launcher via D-Bus | **Parcial**: o comp faz `spawn_command` do `system_actions` (input/actions.rs:1044-1048); o mapeamento para `cosmic-launcher alt-tab` está no settings-daemon. Já trocamos por nosso atalho. | Overlay **interno** de alt-tab é viável (**M**, ~300–600 linhas: `IcedElement` + `FocusStack` MRU + grab de teclado). Overview de janelas interno é **L**. |
| XWayland lazy | **Falso**: eager (xwayland.rs:111; kms/mod.rs:186). | Chave `xwayland = on-demand|always|off` no fork (**S–M**). |
| Patch set_position | 7 arquivos, ~154 linhas +/-; rebase frequente em actions.rs, state.rs, floating/mod.rs. | Custo de manutenção baixo/moderado. |

## 4. Decisão central: fork mantido do cosmic-comp [decidido pelo escopo; detalhes: minha escolha]
Passamos a manter **`cosmic-comp-aenes`** como fork do upstream, como série de **patches versionados** em `patches/cosmic-comp/` (o modelo já adotado para `set_position`), reaplicados pelo PKGBUILD a cada release do COSMIC, com checagem de versão no instalador (spec base §7). Ordem de entrada:
1. **P1 — chaves de aparência** (S–M): `animations: bool|duration_ms`, `shadows_floating`, `shadows_tiled` (já existe), `overview_alpha`, lidas de `CosmicCompConfig` com default = comportamento atual. Perfil leve zera.
2. **P2 — XWayland sob demanda** (S–M). **[decidido 27/10, Diretor]** Entregue como chave `xwayland = always | off`; `on-demand` fica **reservado** (cai em `always` com aviso no log): o smithay da versão (e3d461a) faz o bind dos sockets X e sobe o Xwayland no `XWayland::spawn` (`prepare_x11_sockets`/`X11Lock` são `pub(crate)`). A série de patch no smithay (spawn com sockets em escuta via `-listenfd`) fica **adiada**; reabrir só se a medição do perfil leve/ARM mostrar que o Xwayland ligado custa demais.
3. **P3 — overlay de alt-tab interno** (M): reaproveita `FocusStack` e `shadow_render_element`; aciona por `Action::System(WindowSwitcher)` sem spawn; libera o Alt via grab. O módulo `alttab` do daemon **permanece** como alternativa para o perfil "sem patch" e é desligado quando P3 estiver ativo (o instalador decide pela versão do comp).
4. **P4 — papel de parede desenhado pelo comp** (M) [PDF: elimina `cosmic-bg`, 65 MB]: elemento de fundo lido de um arquivo/shm indicado pelo módulo `wallpapers`; **avaliar** antes se o ganho vale frente a manter `cosmic-bg` fora. **[decidido 27/10, Diretor] NO-GO**: avaliação em `docs/fork-comp/P4-avaliacao.md` (cosmic-bg mede ~63,8 MB PSS, mas rebase em render/mod.rs e workspace.rs e risco de travar o quadro na decodificação pesam mais); `cosmic-bg` continua, sem shim. **Fase futura** (fora desta onda): desenhista de fundo próprio via layer-shell no módulo `wallpapers` — ganho potencial ~64 MB de PSS e viabiliza o shim do `cosmic-bg`.
5. **Não entra na v1:** pixman (L), overview interno (L). Ficam documentados como fase futura.

Anti-meta reforçada: um patch que não aplica **desliga só a função** (P1→perfil leve indisponível, P3→alt-tab volta ao módulo) e o resto segue com o comp upstream (spec base §7).

## 5. Perfis de hardware [decidido: escalabilidade; desenho: minha escolha]
Um **perfil** é um conjunto de chaves (comp + daemon + sessão) escolhido por detecção em runtime, com override manual na janela de configurações:

| Perfil | Detecção | Comp | Daemon/módulos | Sessão |
|---|---|---|---|---|
| **desktop** (esta estação) | render node + ≥ 8 GB | tudo ligado; alt-tab overlay (P3) | todos os módulos; prévias ao vivo; clima | shims desligam painel/launcher/app-library/workspaces/notifications/osd/bg |
| **notebook/iGPU** | render node + bateria presente (UPower) | animações 120 ms, sombras ligadas | prévias por evento (não 1 s); clima com cache longo; `MemoryHigh` menor | idem |
| **sem GPU** | sem render node ou `EGL_MESA_device_software` | **perfil leve**: animações 0, sombras off, overview_alpha 1.0, XWayland off | sem miniaturas ao vivo (estáticas 1/4), sem clima animado, `monitor` só sob demanda | idem; `ICED_BACKEND=tiny-skia` exportado para apps iced |
| **ARM/Raspberry** | `uname -m` aarch64 + < 2 GB | perfil leve + 1 saída até 1080p | módulos mínimos: barra, notificações, quick settings básico; start menu sem prévias | idem |

Regras: a detecção é **pura e testável** (entradas: `/dev/dri/renderD*`, `/sys/class/power_supply`, `/proc/meminfo`, `uname`), vive em `crates/core/src/profile.rs`; cada módulo declara `fn budget(profile) -> ModuleBudget` (memória alvo, intervalo de amostragem, features) e o supervisor **não liga** módulos fora do orçamento do perfil. Testes: tabela perfil × entradas, e um teste de "orçamento total por perfil ≤ meta" (sem GPU: shell ≤ 250 MB PSS; ARM: ≤ 200 MB; desktop: ≤ 350 MB) — metas do PDF adotadas como **alvo**, a validar com `measure-idle.sh`.

## 6. Sessão e prioridade [minha escolha, aprovação prévia]
- **Substituição da sessão por shims** (fluxo INS, já desenhado): painel, launcher, app-library, workspaces, notifications, osd, bg passam a shims que sobem o original só quando o marcador por usuário não lista o componente. Fallback mantém os originais (spec base §7).
- **Launcher**: o start menu + busca já cobre o launcher; o módulo `startmenu` assume `Super` e o nome D-Bus do launcher **só** quando o shim o desativa. App-library funde-se ao start menu (view "Todos").
- **OSD**: o módulo `notifications` já renderiza a pílula (hint `value`); o OSD de volume/brilho do COSMIC (`cosmic-osd`) vira módulo `osd` fino que escuta os mesmos eventos (settings-daemon/D-Bus) e usa a pílula — e os **diálogos polkit** do cosmic-osd ficam no original (shim mantém `cosmic-osd` para polkit) até termos um módulo próprio [minha escolha: segurança].
- **[decidido 27/10, Diretor — revisão após leitura do sistema]** O `cosmic-osd` 1.10 é um processo único (agente polkit, diálogos de energia/sair, confirmação de fone e OSD de volume/brilho): **fica original, sem shim**; o módulo `osd` sai pronto com `DEFAULT_ENABLED=false` (evita pílula duplicada); agente polkit próprio = fase futura. **Sem shim do `cosmic-bg`** (é quem desenha o fundo): decidir após a avaliação do P4. `cosmic-launcher` e `cosmic-app-library` viram **shims preguiçosos fail-open** (instância da sessão ociosa com flock; chamadas sob demanda executam o original com os mesmos argumentos). **Fusão app-library→startmenu ADIADA** para a fatia F6 (UI do start menu; base: view "Todos" de `allapps.rs`). ananicy-cpp 1.2 não tem `LowLatency`: o daemon usa nice −5/best-effort inline (divergência registrada).
- **[decidido 27/10, USUÁRIO]** (a) **Licença:** o projeto é todo do Alexandre, baseado nas fontes COSMIC sem tratamento de fork; não será distribuído — nada a separar por licença (crates greeter* podem ser usados pelo daemon); **antes de qualquer distribuição** fazer a atribuição às fontes. (b) **greeter-daemon:** `GetUsers` tipado substitui `GetUserData`; sem compatibilidade com o greeter do fork (contrato documentado e testado). (c) **cosmic-workspaces:** sem decisão do usuário ⇒ vale o comportamento atual do instalador: o `cosmic-workspaces` é **substituído por padrão** (`keep_workspaces = false`) e a opção `--keep-workspaces` mantém o oficial; sem trabalho extra.
- **ananicy**: entregamos `packaging/ananicy/cosmic-cosmetics.rules`: `cosmic-comp` e `cosmic-cosmetics-holder` = `LowLatency_RT`; `cosmic-cosmeticsd` = `LowLatency` (não RT: módulos de risco não podem monopolizar); `cosmic-cosmetics-logi`, wallpapers = `BG_CPUIO`.
- **Integração no daemon (gargalo):** `bin/daemon` registra todos os módulos existentes com `DEFAULT_ENABLED` por perfil; `[modules.<id>]` na config; `settings-app` lista/ativa; `idle_threads` roda com tudo registrado.

## 7. Não-metas desta expansão
- Compositor próprio; backend pixman; overview de janelas interno (ambos L — fase futura, registrados).
- Reescrever `cosmic-settings`, `cosmic-files`, portais; o `xdg-desktop-portal-cosmic` (75 MB) fica como está.
- Distribuição pública: continua "máquinas do Alexandre", **mas** o código nasce com perfis para que outra máquina (notebook, ARM) funcione com o instalador sem mudança de código.

## 8. Riscos novos
- **Fork do comp** é a primeira vez que mexemos no compositor: um patch errado derruba a sessão. Mitigação: cada patch com teste no compositor aninhado e na VM antes da sessão diária; `RestartPreventExitStatus`/checagem de versão já no instalador.
- **Grab de teclado do alt-tab interno (P3)** não tem precedente no comp.
- **Perfil sem GPU** depende do llvmpipe (Mesa + LLVM JIT ~100–150 MB [PDF]): a meta de 250 MB só se confirma medindo.
- O número do PDF para ganho do painel (400–700 MB) **não bate** com a nossa medição (~250 MB de applets); a meta realista desta estação é **1 181 MB → ≤ 350 MB de PSS**.

## 9. Aceite
1. `cosmic-cosmeticsd` com todos os módulos registrados; `measure-idle.sh` por perfil com os alvos de §5.
2. Patches P1–P3 aplicando limpo no `cosmic-comp` 1.10.x, com testes; P4 avaliado.
3. Sessão com shims: `ps` mostra ≤ 9 processos de shell (comp, holder, daemon, settings-daemon, idle, portal, greeter residente, logi subprocesso, **cosmic-osd original**). **[decidido 27/10, Diretor]** Os shims ociosos (panel, notifications, workspaces, launcher, app-library) não contam; o relatório mede e informa o PSS deles.
4. Perfis: testes puros da detecção e dos orçamentos; execução headless em cada perfil forçado.
5. Revisão final `fable` de tudo; segurança do greeter corrigida (ressalvas I1–I5) antes de instalar.

## 10. Adendo 27/10 — qualidade visual e estabilidade DEVEM AUMENTAR [decidido]
A economia de recursos (§5) **não pode custar** acabamento nem estabilidade. Passam a ser critérios de aceite, não intenções:

**Qualidade visual (meta: igual ou melhor que o COSMIC original em cada perfil)**
- Fidelidade ao tema COSMIC ao vivo (cores, raios, espaçamento, fonte, escuro/claro, alto contraste) em TODA superfície nossa; nenhum "quase igual".
- Texto: antialiasing com subpixel/hinting coerente com o do COSMIC; sem texto borrado em escala 1.25/1.5/2.0 (desenho na escala do buffer, nunca upscale de bitmap).
- Animações: ease-out, 120–240 ms, 60 fps estáveis no desktop (medido por frame callbacks: zero quadro perdido num slide de 240 ms); no perfil leve, animações off mas transições sem "pulo" visual (fade curto ou corte limpo).
- Sem flicker: nada de recriar superfície para redimensionar (usar `ShellHandle::resize` do SHX); popups aparecem no quadro seguinte ao clique; prévias/miniaturas com filtragem (nunca nearest em downscale).
- Ícones nítidos por tamanho (SVG rasterizado no tamanho pedido; PNG do tema só quando houver o tamanho exato); sombras suaves (gaussiana, não box) e cantos com antialias.
- Consistência: um único kit (`engine::ui`) com tokens de espaçamento/raio/elevação; nada desenhado "à mão" fora dele a partir desta onda.
- Gate: por fluxo de UI, **capturas automáticas** (headless via Painter/Pixmap → PNG em `target/visual/`) por tema (claro/escuro/alto contraste) e escala (1.0/1.25/2.0), comparadas com golden images aprovadas pelo Alexandre; qualquer diferença > tolerância falha o teste.

**Estabilidade (meta: zero regressão visível ao usuário)**
- Nenhum módulo pode derrubar o daemon: `catch_unwind` nas fronteiras já existe; passa a ser **obrigatório** teste de "pânico injetado" por módulo (start/draw/callback) provando que só ele reinicia.
- Sem vazamento: teste de longa duração por módulo (10 000 ciclos de evento/redraw, ou 1 h headless no gate da fase) com RSS estável (< 1% de crescimento) e sem threads/fds acumulados.
- Sem flakiness: suíte 10× seguidas verde por crate antes de "concluído"; testes dependentes de tempo têm margem ≥ 5× ou relógio injetado.
- Protocolo Wayland: nenhum caminho pode produzir erro de protocolo fatal (validação ANTES do envio: tamanhos, âncoras, zonas, regiões); fuzz dos validadores puros.
- Entrada externa (D-Bus, arquivos, IPC): fuzz/adversarial obrigatório, timeouts em toda chamada, sem `unwrap` em dado externo.
- Degradação graciosa: toda dependência opcional (compositor sem um protocolo, patch do comp ausente, serviço D-Bus ausente, sem GPU) deixa o módulo funcionando no modo reduzido com aviso, nunca em loop de reinício (cap de restart já existe; testar).
- Revisão: todo fluxo de UI passa por um revisor com lente **visual + estabilidade** além da lente de segurança.
