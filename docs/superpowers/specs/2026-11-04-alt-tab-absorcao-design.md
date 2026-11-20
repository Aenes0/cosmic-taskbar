# Alt+Tab absorvido pelo cosmic-taskbar — design

Data: 2026-11-04 · Estado: rascunho para aprovação · Autor da decisão: Alexandre

## Intenção e porquê

Hoje o Alt+Tab estilo Windows 11 vive num fork do cosmic-workspaces
(`~/Projetos/cosmic-workspaces`, repositório `Aenes0/cosmic-workspaces-aenes`), com pacote Arch,
serviço e atalhos próprios. Tudo o mais da sessão (barra, Iniciar, Central, bandeja, notificações,
clipboard, logi) já vive no repositório unificado `~/Projetos/cosmic-taskbar`. O objetivo é ter **um
programa só** e um **único repositório**: o Alt+Tab passa a ser um **módulo** do `cosmic-taskbar`,
mantendo o comportamento e a leveza que o usuário já aprovou em 23/10.

## Objetivos

1. O Alt+Tab (grade de janelas, coluna de **várias áreas de trabalho**, MRU, mouse, arrasto para
   mover/encaixar) funciona igual ao de hoje, vindo do binário `cosmic-taskbar`.
2. **Leveza acima de tudo.** O módulo não pode pesar de forma permanente: em repouso, sem
   janela, sem captura de tela e sem threads de polling. Meta de referência: o fork isolado mede
   ~24 MB PSS / 16 threads; o módulo dentro do host deve acrescentar **bem menos** que isso em
   repouso (a medir e registrar).
3. Sem atraso perceptível ao abrir: manter painel aos 150 ms segurando Alt, capturas iniciadas no
   atalho, previews prontas em ~60 ms.
4. Se o processo que hospeda o módulo reiniciar, o Alt+Tab pode ficar mudo, **no máximo 2 s**
   (requisito de aceitação, a medir). Durante esse tempo vale o Alt+Tab nativo do COSMIC.
5. Repositório antigo absorvido com **histórico** e **patches**, depois arquivado no GitHub e apagado
   do disco (igual aos outros seis).

## Não-objetivos

- O overview original do COSMIC (Super+W, painel de áreas de trabalho): **sai**. O usuário quer só
  o Alt+Tab com várias áreas de trabalho.
- Não trocar a pilha de renderização, nem reescrever a lógica do switcher.
- Não resolver nesta entrega as pendências antigas (ordem MRU entre reinícios; tela cheia ignorada
  pelo encaixe). Ficam registradas em `gaps.md`.
- Não passar o `cosmic-comp-aenes` para o cosmic-taskbar. O compositor patchado continua pacote
  separado; só seus **patches** e o script de preparo vêm para o repositório (ver abaixo).

## Anti-objetivos (seria falha mesmo funcionando)

- Pesar mais em repouso que o fork isolado de hoje.
- Alt+Tab mudo por mais de 2 s num reinício, ou preso sem nada (sem fallback nativo).
- Perder o histórico de commits do fork.
- Deixar o pacote `cosmic-workspaces-aenes` e o novo caminho disputando o nome D-Bus ou os atalhos.

## Decisões do usuário (como foram dadas)

- Um programa só; o Alt+Tab é um módulo do painel, "porém super leve".
- Pior caso é pesar mais; aceita 1–2 s mudo no reinício, nunca mais que 2 s.
- Repositório antigo: arquivar e apagar como os outros; **trazer histórico e patches**.
- Escopo: "só o Alt+Tab com várias áreas de trabalho" — overview fora.

## Decisões minhas (a conferir)

- **Hospedagem**: o módulo roda dentro do host `central --panels` (um processo único para todas as
  telas, já residente, `TOKIO_WORKER_THREADS=1`, sem polling), em vez de um processo `--switcher`
  próprio. É a opção mais leve. **Risco**: o host é um app iced cujo `update/view/subscription`
  teria de abrigar o estado do switcher; o fork é um `App` iced completo (superfícies layer-shell
  próprias, thread Wayland própria). Por isso a primeira etapa do plano é um **spike** que prova
  (a) que o módulo cabe no host sem alterar o comportamento dos painéis, (b) a medição de repouso e
  (c) o tempo de reinício < 2 s. **Plano B se o spike falhar**: modo próprio `central --switcher`
  (processo separado, com unit e ativação D-Bus), aceitando mais peso residente — voltar ao usuário
  antes de adotar o plano B.
- **Nome D-Bus**: manter `com.system76.CosmicWorkspaces` / `ShowSwitcherOn` etc. durante a migração
  para não mexer no compositor patchado nem nos atalhos; renomear para o domínio
  `io.github.Aenes0.*` do taskbar fica como decisão do construtor, com os atalhos e o guard
  atualizados juntos.
- **Histórico**: o código do fork passa a viver como módulo Rust em `src/switcher/`. O histórico
  entra como **merge de histórico não relacionado** (por exemplo `git filter-repo` com
  reorganização de caminhos no clone do fork, depois `git fetch` + `git merge
  --allow-unrelated-histories` no taskbar) para que `git log --follow` e `git blame` funcionem. O
  método exato é do construtor; o resultado exigido: os 449 commits acessíveis a partir do `main`
  do taskbar e os arquivos do fork nos caminhos finais.
- **Arquivos soltos do fork** (2 sem commit: `src/log_filter.rs` e uma alteração em `src/main.rs`)
  entram junto como parte do trabalho pendente, não se perdem.
- **Fim de vida do antigo**: só depois de a nova versão estar instalada e provada (Alt+Tab ao vivo),
  arquivar `Aenes0/cosmic-workspaces-aenes` no GitHub e apagar `~/Projetos/cosmic-workspaces` do
  disco — **com confirmação do usuário na hora**, por ser irreversível localmente.

## Decisões minhas — resposta ao revisor (a conferir)

- **Limiares do spike** (passa só se todos valerem): acréscimo de repouso do host ≤ **8 MB PSS**,
  **0** threads novas de polling em repouso (thread Wayland só criada na 1ª abertura, ou mantida
  bloqueada sem timeout), comportamento dos painéis inalterado (testes do taskbar verdes e Central
  abrindo normalmente). Falhou em qualquer um → parar e **voltar ao usuário** antes do plano B
  (`central --switcher`); orçamento do plano já prevê as duas rotas.
- **Versões**: o fork usa libcosmic `8cb10bf`, o taskbar `5a8bd94`. Portanto subir o fork para o rev
  do taskbar é um **porte de dependência** (API de iced/libcosmic pode ter mudado), tratado como
  etapa própria e anterior à fusão. O `Cargo.lock` do taskbar não pode mudar para a barra/Central
  (nada de `cargo update`); o porte se adapta ao lock do taskbar, não o contrário. Se o porte exigir
  mexer no lock, parar e avisar.
- **2 s medidos** de "processo do host morto" até "nome D-Bus de volta e `ShowSwitcherOn` respondendo",
  incluindo o intervalo em que o guard devolve o nativo (hoje ~0,6 s para sair e <0,5 s para voltar).
  Vale para o **host `--panels`** (unit própria), não para o reinício de todos os applets do painel.
  O guard continua por `NameOwnerChanged` (via `dbus-monitor`), sem polling.
- **Compositor/protocolo**: sem mudança. O atalho continua chamando `ShowSwitcherOn` por D-Bus; o
  host só precisa ser dono do nome. `COSMIC_ACTIVE_OUTPUT` segue vindo do `cosmic-comp-aenes`.
- **`log_filter.rs` e a mudança em `main.rs`** do fork: trabalho concluído e em uso (o `main()` chama
  `log_filter`), com 5 testes embutidos; entram como estão.
- `git-filter-repo` **não está instalado** no host; alternativa sem ele: `git subtree add` ou
  `git fetch` + `merge --allow-unrelated-histories` com `read-tree --prefix`, aceitando que o
  histórico antigo fique com os caminhos antigos (a rastreabilidade por `--follow` perde-se). Escolha
  do usuário, abaixo.

## Restrições

- Nada de `git commit`/`push` por Claude (regra do usuário): entrega = árvore de trabalho pronta,
  build e testes verdes; o usuário commita. Isso inclui o import do histórico: o construtor prepara
  o passo (comandos/branch) e o usuário o executa, ou pede explicitamente.
- `git` só existe no host: `flatpak-spawn --host git -C …`.
- O `[patch]` do `cosmic-protocols` com os patches `0001`/`0002` (set_position, bind v5) precisa
  valer para o crate do taskbar inteiro, sobre o mesmo rev `c0cff4d` que ele já usa. O `cctk` da
  barra continua compilando. `scripts/prepare-build.sh` (clona upstream em `build/` e aplica os
  patches) migra junto; `build/` fica fora do git.
- O guard (`guard.sh`) e `alt-tab-shortcuts.sh` continuam a garantir que o Alt+Tab nativo volta
  quando o dono do nome D-Bus some; precisam apontar para o novo executável/unit. O unit do guard
  vira parte do `install.sh` do taskbar.
- `setup.sh` / `install.sh` do taskbar ganham a instalação do guard e dos atalhos; a barra e o
  host continuam sendo reiniciados como hoje.
- Requisitos de qualidade já do projeto: `cargo test` verde (os ~61 testes do fork migram), testes
  shell do fork (`shortcuts-test.sh`, `guard-test.sh`, `prepare-build-test.sh`) migram para `tests/`
  do taskbar e passam.

## Pacote e instalação

- O pacote Arch `cosmic-workspaces-aenes` deixa de existir; o caminho do Alt+Tab no sistema passa a
  ser o binário do `cosmic-taskbar` (`~/.local/bin` ou `/usr`, conforme o `install.sh`).
  A sessão do COSMIC lança o `/usr/bin/cosmic-workspaces` oficial: é preciso impedir que o oficial
  (Super+W e dono do nome D-Bus) volte a subir, ou garantir que não dispute o nome. Como proceder
  (remover o pacote aenes e voltar o oficial? override do serviço?) é do construtor, **mas tem de
  ser comprovado ao vivo** que o Super+W não sobe nada que se choque com o nosso.
- `cosmic-comp-aenes` (compositor patchado) segue instalado como está; a origem dos seus patches
  muda de lugar (`patches/cosmic-comp/`, `PKGBUILD` em `packaging/arch/`).

## O que fica para o construtor

- Forma exata de embutir o estado do switcher no app do host (enum de mensagens aninhado,
  subscription condicional, ciclo de vida das superfícies layer-shell e da thread Wayland criada
  só na primeira abertura, ou mantida mínima).
- Quais partes do `main.rs` de 1830 linhas do fork pertencem ao switcher (a coluna de áreas de
  trabalho, SIDEBAR_W, `Mode::Switcher`, `capture_filter`) e quais são só do overview (remover
  `view/mod.rs` do overview, `workspace_bar`, `toplevels` layouts, `dnd.rs` se não usados pelo
  arrasto do switcher; i18n reduzido às strings usadas).
- Método do import do histórico; renomear ou não o nome D-Bus; como o host sinaliza prontidão ao
  guard.
- Como medir peso (PSS, threads, fds) e tempo de reinício de forma repetível; usar
  `scripts/measure.sh` do taskbar como base.

## Critérios de aceitação

1. `cargo build --release` e `cargo test` verdes no taskbar; testes shell migrados verdes.
2. Alt+Tab ao vivo: abre aos 150 ms, previews ~60 ms, várias áreas de trabalho, MRU, mouse, arrasto
   com encaixe, mudar de monitor — igual ao de 23/10 (conferência do usuário).
3. Repouso: medido e registrado (PSS/threads/fds do host antes e depois), com o aumento na casa de
   poucos MB; nenhuma thread nova de polling.
4. Reinício do host: Alt+Tab mudo ≤ 2 s medido; o nativo cobre o intervalo; sem atalho preso.
5. Histórico dos 449 commits presente; `gaps.md` atualizado com as pendências herdadas.
6. Fork antigo arquivado e apagado **só depois** de 1–5 e da confirmação do usuário.
