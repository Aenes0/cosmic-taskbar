# Lacunas conhecidas

- **Código ainda duplicado entre a barra e o menu** (plano, Fase 2): lista de janelas (toplevels),
  cache e pré-rasterização de ícones, tokens de estilo (raios, vidro, sombras) e o cartão de
  prévia. Molas, interruptor de movimento e idioma já são compartilhados (`src/shared/`); a
  captura de miniaturas está sendo movida para lá.
- **Textos**: `bar/texts.rs` e `start/texts.rs` ainda são dois módulos, com `untitled` e "fechar
  todas as janelas" repetidos.
- **Fixados**: a barra grava `favorites` em `com.system76.CosmicAppList`; o menu só semeia a
  partir dele e depois usa a própria chave. Não são compartilhados em tempo real.
- **Barra: 36 threads por processo** (tokio com um worker por núcleo). O Alt+Tab ficou com 16 ao
  limitar o tokio a 2 workers; falta o mesmo aqui.
- **Instalação**: depois de `install.sh` o serviço `--menu` antigo continua vivo com o arquivo
  apagado até ser reiniciado (`pkill -f "cosmic-taskbar --menu$"`).
- **Prévia no pior caso**: janelas de título com muitas letras largas ou ideogramas podem estourar
  a estimativa de largura (0,56 do tamanho da fonte por letra) e ser cortadas antes do esperado.
- **Menu: fade de conteúdo**: o menu desliza inteiro, mas não esmaece (um menu não esmaece como
  um todo sem apoio do compositor).
- **Vault Brain**: nota de feature e ADR da unificação ainda não escritas (servidor MCP do
  Obsidian fora do ar na sessão de 30/10).
