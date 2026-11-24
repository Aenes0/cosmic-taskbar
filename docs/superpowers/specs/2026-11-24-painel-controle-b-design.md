# Painel de Controle — redesenho "B · Central de comando"

Data: 2026-11-24 · Status: rascunho para aprovação · Mockups: https://claude.ai/artifact/7jbuYGLPDnbZieAQny7nuZ
(artboards `Direcao-B`, `B-Energia`, `B-Busca` e o componente `Linha`). Prints do estado atual: `target/prints/antes/*.png`.

## 1. Intenção

O Painel de Controle (`cosmic-taskbar control-panel`, `src/control_panel/`) junta os ajustes do projeto inteiro,
mas hoje cansa: abas longas, fileiras de chips, títulos repetidos, ajustes fora do lugar, três jeitos diferentes de
"salvar" e um aviso único sem tipo. O usuário marcou os quatro incômodos como prioritários:

1. abas longas demais;
2. não saber o que foi salvo e o que foi aplicado;
3. dificuldade de achar um ajuste;
4. visual inconsistente.

O redesenho adota a direção **B · Central de comando**: uma tela inicial de **Visão geral**, um **trilho de categorias**
à esquerda, **abas de seção** no topo, **busca global Ctrl K** e um **vocabulário único de controles**.

## 2. Objetivos, não-objetivos e anti-objetivos

**Objetivos**
- Navegação em três níveis: trilho (Início + 5 categorias) → abas de seção → grupos da página.
- Visão geral com alertas acionáveis, métricas ao vivo, cartões de estado com controle direto, ações rápidas e
  alterações recentes.
- Busca Ctrl K (paleta) sobre todos os ajustes, ações e páginas do COSMIC Settings (estas abrem o app nativo).
- Um componente de **linha de ajuste** (rótulo + descrição + controle) usado em todas as páginas; listas longas de
  chips viram `dropdown`; escolhas de 2 a 4 opções viram controle segmentado.
- Uma regra de salvamento: **tudo grava na hora** (já é assim); o que precisa de um passo extra para valer no
  sistema (aplicar CPU/GPU no hardware, reiniciar o serviço do OSD) aparece numa **barra "Aplicar"** fixa no rodapé
  da página enquanto houver pendência.
- Avisos tipados (sucesso / erro / informação) com sumiço automático e, quando couber, "Desfazer".
- Termos técnicos com uma linha de explicação (EPP, RAPL PL1/PL2, ASPM, ALPM, CCD1, Persistence Mode…).
- `views.rs` (5173 linhas) quebrado em módulos por seção.

**Não-objetivos**
- Não reimplementar páginas do COSMIC Settings (arranjo de monitores, atalhos, Bluetooth, som, usuários…).
  A paleta só **indexa** e **abre** essas páginas (`cosmic-settings <página>`).
- Não mexer na aba **Tela de Login** além de ela aparecer dentro da moldura nova (ver §6).
- Não criar tema claro nem novas configurações de usuário além das listadas aqui.

**Anti-objetivos (falha mesmo que "funcione")**
- Perder ou reinterpretar qualquer configuração salva (`power-profiles.conf`, `screenshot.conf`, config do OSD,
  configs do central/monitor/notificações, `greeter.toml`).
- Aplicar no hardware ou reiniciar serviço sem o clique do usuário (mudaria comportamento e pode pedir senha várias
  vezes).
- Métricas ao vivo gastando CPU com o painel fechado ou minimizado.
- Cores fixas fora do tema COSMIC (o verde do `status_badge` atual, a paleta `TileTone`).

## 3. Restrições

- **Não tocar:** `greeter/`, `extras/greeter/`, `install.sh`, o bloco da Tela de Login em `views.rs` (de
  `fn style_label` até o fim de `fn login_view`, incluindo `login_live_preview`; as linhas mudam enquanto a
  entrega do greeter trabalha), a lista de estilos/parsers de `GreeterConfig` em `data.rs`, o formato de
  `~/.config/cosmic-taskbar/greeter.toml`.
- Os helpers antigos que o login usa (`status_badge`, `card_box`, `action_btn`, `toggle_row`, `small_chip`…)
  **ficam como estão** num módulo legado usado só pelo login, para o visual dele não mudar. As seções novas usam
  helpers novos. Mover o bloco do login de arquivo admite trocar só as linhas `use`.
- **Execução só depois que a entrega do greeter terminar** (ela tem `views.rs` e `data.rs` modificados e não
  commitados na `main`). Branch e worktree são definidos nessa hora (G0).
- Stack: libcosmic do git (rev fixada no `Cargo.lock`; os patches de `third_party/` estão desligados). Ela já tem
  `dropdown` (abre em overlay), `segmented_control` (já usado no Monitor), `popover`, `toaster` (só texto, sem
  tipo) e `widget::settings`. Nada de dependência nova.
- Ambiente: build e prints pelo host (`flatpak-spawn --host`), prints em `target/prints/`.
- Sem commit (regra do usuário). TDD onde houver lógica.

## 4. Abordagens consideradas

| Abordagem | Como é | Troca |
|---|---|---|
| A · Nativo refinado | Barra lateral agrupada, coluna única, avançado recolhido | Mais barata e próxima do COSMIC Settings; não resolve "achar ajuste" além da busca da lateral, não tem visão de estado |
| **B · Central de comando** (escolhida) | Visão geral + trilho de categorias + abas de seção + Ctrl K | Resolve os quatro incômodos; custa a Visão geral e a paleta |
| C · Ultrawide em colunas | Lateral + cartões em 2 colunas + trilho "Nesta página" | Aproveita o DP-3, mas fica apertada no mínimo de 780 px e não resolve "salvo ou aplicado" melhor que A |

Sobre puxar todas as configs do COSMIC Settings para dentro: descartado (escopo de semanas, quebra a cada versão do
COSMIC, dois editores da mesma chave). O meio-termo escolhido é indexar o nativo na paleta.

## 5. Arquitetura de informação

Trilho (ordem fixa): **Início**, **Sistema**, **Aparência**, **Ferramentas**, **Proteção**, **Manutenção**.

| Categoria | Seções (abas) | Mudança em relação a hoje |
|---|---|---|
| Sistema | Energia & Bateria · Monitores & Janelas · Rede & VPN · Câmeras & Áudio | "Monitores & Visuais" → "Monitores & Janelas" (sem o recorte); "Rede" → "Rede & VPN" e passa a ter o único controle de troca de MAC |
| Aparência | Barra & Tray · Central (Direita) · Monitor (Esquerda) · Notificações · OSD de Dispositivos · Tela de Login | OSD sai de "Avançado" e fica ao lado de Notificações (de onde herda posição e estilo) |
| Ferramentas | Captura de tela | **Nova seção**: o "Recorte de tela" sai de Monitores; busca encontra por "print", "recorte", "screenshot" |
| Proteção | Segurança & Privacidade · Backup & Sincronização | MAC rotativo sai daqui (vira atalho para Rede) |
| Manutenção | Pacotes & Opcionais · Atualizações & Serviços | MAC rotativo sai de Opcionais |

- `--section X` continua aceitando todos os valores de hoje (`Section::from_arg`) e passa a aceitar `capture`,
  `captura`, `print`, `screenshot` e `home`/`inicio`. Sem `--section`, abre em **Início**.
- O título de cada página é o rótulo da aba (some o `title3` duplicado e os títulos divergentes como
  "Câmeras & Privacidade").
- Ações duplicadas viram uma só: "Restaurar tiles" (Central), "Testar OSD" (fica no OSD; Notificações mantém
  "Enviar teste"), "Abrir Mission Center", "Corrigir ícones PWA" (fica em Monitores; Opcionais só "Abrir").

## 6. Moldura (shell)

- **Cabeçalho** (headerbar libcosmic): título "Painel de Controle" à esquerda, campo de busca que abre a paleta no
  centro, indicador de salvamento ("Tudo salvo" / "1 ajuste para aplicar") à direita, botões de janela.
- **Trilho** de 92 px: ícone em pílula + rótulo; item ativo com pílula em destaque; botão "Atualizar status" no pé
  (substitui o do rodapé da lateral). Um ponto âmbar no ícone da categoria quando ela tem alerta (serviço parado,
  pendência de aplicar).
- **Página de seção:** trilha "Início › Categoria", abas sublinhadas das seções da categoria, cabeçalho da página
  (ícone, título, subtítulo, selo de estado opcional, ações à direita), atalhos "Ir para" (um por grupo +
  "Avançado"), grupos em cartões de coluna única (`max_width` ~900).
- **Avançado recolhido:** grupos marcados como avançados (Energia: CPU, GPU, Plataforma, Kernel; Segurança:
  Blindagem avançada, Auditoria) ficam atrás de um botão "Mostrar ajustes avançados" que lista o que há dentro.
  Substitui `TogglePowerAdvanced` e `ToggleSecurityAdvanced` sem mudar o que eles escondem.
- **Barra "Aplicar"** (rodapé fixo da página, só quando há pendência): texto do que está pendente + botão primário.
  - Energia: aparece quando o plano em edição tem ajuste gravado e ainda não aplicado por este painel (CPU, GPU,
    plataforma e taxa de atualização; os tempos de tela já vão ao COSMIC Idle na hora quando o plano é o ativo).
    Se o plano em edição é o ativo: "Aplicar no hardware". Se não é: "Ativar e aplicar <plano>". Os dois seguem o
    mesmo caminho do `PowerSaveAndApply` atual, que também torna o plano ativo, sem o `save()` redundante.
    "Aplicado" significa o `tuning` que este painel aplicou por último, guardado em memória; trocar o plano ativo
    (`SetPowerProfile`, cartão da Visão geral) zera a pendência daquele plano, porque o daemon aplica ao trocar.
  - OSD: aparece quando o arquivo mudou desde o último reinício do serviço → "Aplicar (reinicia o OSD)" (mesmo
    caminho do `OsdSaveAndRestart`).
  - Somem: "Salvar sem aplicar" (`PowerSaveOnly`), "Salvar rascunho" (`OsdSaveOnly`), "Salvar e aplicar plano",
    "Salvar e reiniciar". Tela de Login mantém os botões dela.
- **Avisos tipados:** pilha própria no canto inferior (o `toaster` do libcosmic não tem tipo nem ícone), tipo
  (sucesso, erro, info, em andamento) com ícone e cor do tema, some sozinho em ~5 s (erro fica até fechar),
  "Desfazer" opcional. As mensagens que hoje embutem "✓/✗" no texto passam a declarar o tipo. `busy` passa a desabilitar o botão que
  disparou a ação e mostrar "em andamento" no aviso, em vez do texto no rodapé.
- **Tela de Login:** renderiza o `login_view` atual, intacto, dentro da moldura nova (sem o `page_banner` antigo se
  isso não exigir tocar no código dela; se exigir, fica com banner duplicado até a entrega do greeter acabar).

## 7. Visão geral (Início)

Ordem de cima para baixo:

1. Título "Visão geral" + linha de hardware (CPU · GPU · nº de monitores · "atualizado agora").
2. **Alertas** (0 ou mais), só quando há algo a resolver, cada um com "Detalhes" (vai à seção) e uma ação direta.
   Fontes iniciais: serviço rastreado parado, proteção abaixo do recomendado, backup atrasado ou destino
   desmontado, pendência de aplicar.
   - "Parado" = unidade **carregada** (`LoadState=loaded`) e inativa. Unidade ausente ou não instalada não gera
     alerta.
   - "Backup atrasado" = último disparo do timer (`LastTriggerUSec`) há mais de 26 h (constante). Último e
     próximo horário vêm de `systemctl show` do timer, sem abrir o repositório restic.
3. **Métricas ao vivo** (4 cartões): CPU (uso, temperatura, frequência), GPU (uso, temperatura, watts), memória
   (uso), consumo (W e R$/h pela tarifa da seção Monitor). Barra fica âmbar a partir de 70%.
   - Cadência: 2 s com a Visão geral aberta e a janela em foco; 10 s com a janela sem foco (o Wayland não informa
     "minimizada"); parada em qualquer outra seção.
   - Fonte: o cálculo puro que hoje vive dentro do Monitor da esquerda (uso de CPU por delta, watts por delta do
     RAPL, total) é extraído para um módulo compartilhado **sem mudar o comportamento do Monitor**, com testes; a
     GPU vem do valor que o host já publica (`gpu_share`), sem carregar o NVML no processo do painel.
4. **Cartões de estado** (3 × 2): Energia (com seletor de plano Economia/Equilibrado/Desempenho direto no cartão),
   Proteção (nota + "Verificar agora"), Backup (último/próximo + "Executar agora"), Rede (conexão + VPN),
   Monitores (nº e principal), Serviços (ativos/total + "Reiniciar parados", que só reinicia unidades de usuário;
   unidades de sistema paradas levam à seção, sem pedir senha em lote). Cada cartão tem "Abrir".
5. **Ações rápidas** (grade 2 colunas): notificação de teste, testar recorte, executar backup, verificar segurança,
   reiniciar painel, bloquear tela.
6. **Alterados recentemente** (3 últimos, desta sessão do painel, em memória): rótulo → valor, seção, quando.
   "Desfazer" só nas mudanças simples de valor que gravam em arquivo (interruptor, dropdown, segmentado, slider):
   volta ao valor anterior pelo mesmo `Message`. Restaurar padrões e ações com efeito no sistema
   (`SetPowerProfile`, monitor principal, espaçamento de janelas, ações de segurança) aparecem sem "Desfazer".

## 8. Busca Ctrl K (paleta)

- Abre por Ctrl K, pelo campo do cabeçalho ou por "/" quando nenhum campo tem foco; Esc fecha.
- Resultados em três grupos, no máximo 5 / 3 / 3: **Ajustes** (rótulo da linha + "Seção › Grupo"; Enter vai à
  seção, abre o avançado se preciso e rola até o grupo), **Ações** (botões de ação das páginas; Ctrl Enter
  executa), **COSMIC Settings** (páginas nativas; Enter roda `cosmic-settings <página>`).
- Busca sem acento e sem diferença de maiúsculas, por rótulo, grupo, seção e palavras-chave (as `keywords()` de hoje
  + sinônimos novos). ↑ ↓ navegam, primeiro resultado já destacado.
- O índice é **dado declarado** (lista estática de entradas com seção, grupo, rótulo e palavras-chave), não
  extraído da árvore de widgets, para ser testável.

## 9. Vocabulário de controles (linha de ajuste)

Uma linha = rótulo (14 px, médio) + descrição opcional (12,5 px, 65% de opacidade) à esquerda; controle à direita.
Tipos: interruptor; dropdown; segmentado (2–4 opções); slider (rótulo + valor em selo à direita + mínimo/máximo
abaixo; valor 0 com texto como "Padrão" / "Livre" / "Máximo do hardware"); botão(ões); chave-valor (mono);
estado (pílula com tom); campo de texto (+ botão); lista de itens; cartões de perfil; grade de ícones da barra;
prévia de notificação; amostras de cor; nota informativa.

Tokens: espaçamentos só de `theme::spacing()` (fim dos `spacing(4|6|8|10|12)` soltos); raios do tema; tons de
estado (ok, atenção, erro, info, neutro) derivados das cores do tema COSMIC (`success`, `warning`, `destructive`,
`accent`), nunca RGB fixo. Os mockups usam Inter e um ciano; o app usa a fonte e o destaque do sistema.

Mapeamento de chips → controles (lista completa fica no plano): minutos de tela/bloqueio/suspensão, EPP, teto de
frequência, taxa de atualização, margem/espaço de janelas, prioridade de foco, tempo/posição/linhas de notificação,
intervalos do Monitor e do OSD, ferramenta do Satty, quantidade/ordenação de processos → **dropdown**.
ASPM, ALPM, formato de notificação, espaçamento da barra, touchpad, termômetro, ação pós-captura, provedor de DNS,
cartão de clima → **segmentado**.

## 10. Conteúdo por seção

O conteúdo de cada seção é o de hoje (inventário completo no mockup, que usa os mesmos ajustes e rótulos), com estas
mudanças além das de §5 e §6:
- Energia: no topo, o plano **ativo** (cartões de perfil, como hoje) e, logo abaixo, um seletor sempre visível
  **"Editando: [plano ▾]"** que começa no plano ativo. Todos os grupos por plano (Tela e bloqueio, CPU, GPU,
  Plataforma) mostram e gravam o plano em edição, e o título de cada um diz qual é ("Tela e bloqueio ·
  Desempenho"). Os 4 tempos e a taxa de atualização ficam em "Tela e bloqueio"; Bateria e Periféricos vêm antes
  do avançado; "Kernel, módulos e regras" vai para o avançado.
- Segurança: o provedor de DNS vira um segmentado com a explicação de cada um; selo "Bem protegido · N/100" no
  cabeçalho; "Kill switch" com botão de estilo destrutivo.
- Notificações: a prévia ao vivo é o primeiro grupo; as cores viram amostras + campo hex.
- Rede & VPN: ganha "Trocar endereço MAC periodicamente" (estado, "Trocar agora", ativar/desativar).
- Atualizações & Serviços: a lista de serviços alimenta o alerta e o cartão "Serviços" da Visão geral.

- Rede & VPN: a troca de MAC usa o caminho `RunAction` com pkexec que Segurança usa hoje; o caminho por terminal
  com sudo de Opcionais sai.

## 11. Comportamento a preservar (testes de regressão)

Não existe harness que monte o `ControlPanel` e várias mensagens gravam no `~/.config` real. Em vez de criar esse
harness, a lógica nova e a preservada ficam em **funções puras testáveis**: round-trip das configs, `from_arg`,
índice e ranking da busca, pendência de "Aplicar", fila de avisos, histórico de recentes e desfazer, cálculo das
métricas extraído do Monitor, classificação de serviço parado e de backup atrasado.

- Cada `Message` que grava hoje continua gravando no mesmo arquivo, com o mesmo formato.
- `persist_power_config(true)` continua sincronizando o COSMIC Idle só quando o plano editado é o ativo.
- Hex inválido de notificação continua preservado no campo até ficar válido.
- `SettingsChanged` continua recarregando as configs externas.
- `--section` com os valores antigos abre a mesma seção (com o nome novo quando mudou).

## 12. Decisões tomadas na conferência contra o código

Decididas por Claude (o usuário confere na leitura): ajustes 2 a 9 da conferência, refletidos em §3, §6, §7, §10 e
§11 — helpers do login preservados num módulo legado; libcosmic do git e pilha própria de avisos; cadência das
métricas sem depender de "minimizada"; extração do cálculo do Monitor; testes por funções puras; backup pelo timer
com limite de 26 h; "parado" só para unidade carregada e "Reiniciar parados" só para unidades de usuário;
"Desfazer" só em mudanças simples de valor; taxa de atualização conta como pendência de aplicar.

Decidida pelo usuário: o seletor "Editando: [plano]" fica sempre visível e a barra aplica o plano em edição,
ativando-o se preciso (mantém a capacidade atual do `PowerSaveAndApply`).

## 13. O que fica a critério de quem implementa

- Divisão exata de `views.rs` em módulos (sugestão: `views/{shell,home,palette,row,…}.rs` + um arquivo por seção,
  com `login.rs` movido sem alteração de conteúdo).
- Implementação da paleta como overlay/popover do libcosmic.
- Ordem das ondas, desde que cada onda termine com `cargo build` e `cargo test` verdes e prints.

## 14. Pronto quando

Redesenhado conforme este documento, aprovado pelo usuário com prints antes/depois (`target/prints/antes/` e
`target/prints/depois/`), `cargo build` e `cargo test` verdes, sem commit, relatório em português com arquivos,
testes, prints e a branch.
