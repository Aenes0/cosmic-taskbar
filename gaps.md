# Lacunas conhecidas

- **Compartilhado até aqui** (`src/shared/`): molas e interruptor de movimento, idioma dos textos e
  a captura de miniaturas (a do menu, que a barra passou a usar).
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
  Obsidian fora do ar na sessão de 30/10).
