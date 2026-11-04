# Lacunas conhecidas

- **Compartilhado até aqui** (`src/shared/`): molas e interruptor de movimento, idioma dos textos
  (português, francês, inglês; antes 6 cópias da regra), a inicialização do log (antes 3), a
  captura de miniaturas (a do menu, que a barra passou a usar) e o cartão de prévia.
- **Central e logi-osd (03/11)**: importados com histórico em `src/central/` e `src/logi/`; o
  Central foi portado do libcosmic d4d71fd para o 5a8bd94 (2 mudanças: ação de superfície com o
  tipo da mensagem). Memória total 263 -> 232 MB (PSS+SwapPss), threads 145 -> 135.
- **Avisos do logi-osd sem o estilo próprio**: ele manda dicas `x-estilo`, `x-tom`, `x-pontos` que
  só o daemon `notificacoes` (depreciado, nunca em uso) desenhava; o daemon do COSMIC as ignora e
  mostra uma notificação comum.
- **Registros DIAG em `trace`** (mudos por padrão), 18 linhas: na bandeja
  (`src/central/tray/components/app.rs`, investigação do travamento;
  `RUST_LOG=cosmic_taskbar::central::tray=trace` os liga) e na barra (`src/bar/app.rs`, prévia presa
  na taskbar; `RUST_LOG=cosmic_taskbar::bar=trace`). Remover quando cada investigação for encerrada.
- **Bandeja reconstruída (03/11)**: popups pela máquina `tray/components/popups.rs` (confirma pelo
  `Opened`, sem destroy às cegas); menu de ícone da caixa como filho dela; destroy de tooltip nunca
  aberto filtrado (`forward_destroy`). A "loteria" do ^ era do `cosmic-panel` (popup com grab
  destruído pelo applet deixava entrada morta): corrigido no fork `~/Projetos/cosmic-panel`
  (branch `aenes`, pacote `cosmic-panel-aenes`). **Pendente: arrastar ícones não funciona** (sem
  log; o fork também corrigiu o grab de DnD preso, que travava a barra).
- **Painel de medições lê a GPU pela libnvidia-ml** (temperatura e uso): é leitura de sensor, não
  renderização.
- **Lista de janelas (toplevels): decidido NÃO compartilhar (30/10).** O que as duas têm de igual é
  só a boilerplate dos `impl` do `cctk` (~60-80 linhas de métodos quase vazios); o resto tem
  formato diferente (a barra recebe eventos um a um num objeto grande com saídas, workspaces e
  tokens; o menu entrega um retrato da lista a cada mudança). Unificar pediria uma abstração no
  meio, com risco nos dois lados e nenhum ganho de execução (processos separados, conexões
  separadas).
- **Ainda duplicado e com valor**: o cartão de prévia (a seção "Abertos" do menu usa um cartão
  próprio, `start/menu/preview_card*.rs`, `preview_ui*.rs`, ~400 linhas, no estilo antigo; a barra
  tem o cartão novo, só miniaturas) e os tokens de estilo (raios, cores). Passar o menu para o
  cartão da barra muda o visual do menu: precisa do OK do usuário.
- **Medido (30/10)**: barra parada 0% de CPU; varrendo os ícones com o mouse, mediana 12% e pico
  32% de um núcleo a 60 quadros/s (zoom). Arredondar o ícone para pixel inteiro não mudou nada.
  Baixar o animador do zoom para 30 quadros/s deve cortar pela metade, ao custo de suavidade.
- **Textos**: `bar/texts.rs` e `start/texts.rs` ainda são dois módulos, com `untitled` e "fechar
  todas as janelas" repetidos.
- **Fixados**: a barra grava `favorites` em `com.system76.CosmicAppList`; o menu só semeia a
  partir dele e depois usa a própria chave. Não são compartilhados em tempo real.
- **Instalação**: depois de `install.sh` o serviço `--menu` antigo continua vivo com o arquivo
  apagado até ser reiniciado (`pkill -f "cosmic-taskbar --menu$"`).
- **Prévia no pior caso**: janelas de título com muitas letras largas ou ideogramas podem estourar
  a estimativa de largura (0,56 do tamanho da fonte por letra) e ser cortadas antes do esperado.
- **Menu: fade de conteúdo**: o menu desliza inteiro, mas não esmaece (um menu não esmaece como
  um todo sem apoio do compositor).
- **Vault Brain**: nota de feature e ADR da unificação ainda não escritas (servidor MCP do
  Obsidian fora do ar nas sessões de 30/10 e 03/11).
- **Arquivos antigos em ~/.local/bin**: `cosmic-ext-applet-tray`, `logi-osd.antes-notificacoes` e
  `print-recorte.antes-notificacoes` não são usados por nada.
- **libcosmic sem `rev` no Cargo.toml** (o pin `5a8bd94` só existe no `Cargo.lock`): um `cargo update`
  sem cuidado pode saltar de versão, como aconteceu ao tirar o `[patch]` (foi para `95236a9` e não
  compilou). Não dá para pôr `rev` só no nosso: os crates de acessibilidade do cosmic-settings
  pedem o libcosmic pelo git sem `rev`, e duas fontes viram duas cópias, com tipos que não se
  encaixam. Para atualizar: `cargo update -p libcosmic --precise <commit>`.
- **Arquivos grandes**: `src/bar/app.rs` tem ~3 000 linhas; `src/central/tray/components/app.rs` e
  `src/central/panels/mod.rs` vieram assim do Central. Dividir só quando houver mudança neles.
