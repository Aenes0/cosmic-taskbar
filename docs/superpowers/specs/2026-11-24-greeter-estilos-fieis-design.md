# Greeter: 5 estilos fiéis a cada sistema — design

Data: 2026-11-24 · Projeto: cosmic-taskbar (`greeter/`, `src/control_panel/`, `extras/greeter/`, `install.sh`)
Protótipo aprovado: `.superpowers/brainstorm/195761-1795523755/content/estilos-v1.html` ("pode seguir tudo aprovado").

## Intenção

O cosmic-taskbar é o único projeto do usuário para o desktop. O aenes-greeter (fork do cosmic-greeter, em
`greeter/`) desenha a tela de bloqueio (processo do usuário, lançado pelo `cosmic-session` via
`~/.local/bin/cosmic-greeter`) e, quando `apply-greeter.sh` for aplicado, a tela de login do boot. O usuário quer
as 5 skins escolhidas no Painel de Controle **bonitas e fiéis a cada sistema, reconhecíveis na hora**, e que
todas as opções do Painel realmente funcionem.

## Objetivos

1. Redesenhar os 5 estilos de `login_ui::screen_styled` seguindo o protótipo aprovado:
   | valor em `greeter.toml` | nome no Painel | layout |
   |---|---|---|
   | `win11` | Windows 11 | fundo com vidro acrílico; relógio Semibold centrado no topo, data abaixo; avatar redondo grande, nome, campo retangular (raio 4) escuro com linha de foco na cor de destaque e botão-seta; "Opções de entrada" (menu de sessão) abaixo; ícones de rede/bateria/energia no canto inferior direito |
   | `android` | Android 16 (Pixel) | Material 3 Expressive: cores tonais derivadas do papel de parede; linha "num relance" no topo-esquerdo (data + bateria); relógio gigante em 2 linhas (HH/MM) centrado; campo em pílula tonal clara com ícone de cadeado; botões de canto quadrados-arredondados (energia à direita, sessão/teclado à esquerda); barra de status mínima (hora, rede, bateria) |
   | `macos` | macOS Tahoe | data pequena acima de relógio enorme, arredondado e translúcido; avatar pequeno + nome + campo em pílula de vidro na base; dica pequena abaixo; ícones mínimos no canto superior direito |
   | `ios` (novo; antigo `editorial`) | iOS 26 | barra de status (hora à esquerda, ícones à direita); data acima de relógio gigante e pesado com degradê de vidro; campo de vidro em pílula; dois botões redondos de vidro nos cantos inferiores (energia e sessão); barrinha inferior estilo iPhone |
   | `ipados` (novo; antigo `sidebar`) | iPadOS 26 | data e relógio grandes no canto superior esquerdo; coluna de widgets de vidro abaixo (dados reais, ver item 6); campo de vidro centrado na base; barrinha inferior; papel de parede livre à direita |
2. **Cartão respeita o Painel**: `card_opacity = 0` significa sem cartão em todos os estilos. Remover os
   mínimos por estilo (hoje android 0.42, editorial 0.45, sidebar 0.72; android força raio ≥ 24). Elementos que
   fazem parte da identidade do estilo (pílulas de vidro do campo, botões de canto, widgets) não são "cartão" e
   continuam visíveis.
3. **Desfoque real** (`blur_radius` hoje é ignorado): o greeter desfoca o papel de parede em runtime, nos dois
   modos, fora da thread de UI, com cache em disco.
4. **Tema claro de verdade**: `theme = "light"` troca toda a paleta (texto, campo, vidro, ícones), não só o fundo
   do cartão. Contraste mínimo WCAG AA (4.5:1) para texto do campo e da data sobre o vidro/cartão.
5. **Troca de papel de parede por tempo**: com `wallpaper_mode = "rotate_interval"`, a imagem troca a cada
   `wallpaper_interval_mins` também com a tela aberta (bloqueio e login). No login, a escolha inicial deixa de ser
   sempre o índice 0.
6. **Widgets só com dados reais** (decisão do usuário: "bateria + rede"): iPadOS mostra um widget por
   periférico com bateria (mouse, teclado, fone…) e um widget de rede com o nome da conexão/SSID.
   Hoje os dados NÃO existem: `upower.rs:36,47-50` só lê `get_display_device` (desktop não tem) e
   `networkmanager.rs:38-101` só entrega o nome do ícone. Ampliar as consultas aos mesmos serviços:
   UPower `EnumerateDevices` (filtrando periféricos com `Percentage`) e NetworkManager (`Id` da conexão
   ativa primária / SSID). Widget sem dado não aparece. A linha "num relance" do Android usa os mesmos dados.
   Sem clima e sem notificações (não há fonte de dados; fica fora).
7. **`default_user` e `session`**: hoje só funcionam indiretamente — `apply-greeter.sh:681-717` grava
   `last_user`/`users` no cosmic-config do usuário do greeter, e o greeter sobrescreve `last_user` a cada
   login (`greeter.rs:1890-1917`). A lógica passa para o greeter: `StyleConfig` lê `default_user`/`session`;
   se `default_user` está preenchido e existe, ele é sempre o pré-selecionado (vence o último usuário); vazio =
   último usuário. `session` casa com o `Name` localizado do `.desktop` (`greeter.rs:367`); se não existir,
   usa a última/primeira sessão. **Corrigir o `todo!()` de `greeter.rs:1590`**, que derruba o greeter com sessão
   inexistente. O script para de gravar `last_user`/`users`.
8. **Prévia do Painel** (`src/control_panel/views.rs`, `login_live_preview` e lista de estilos em `login_view`)
   acompanha os novos layouts e nomes.
9. **Fontes**: cada estilo tem uma fonte nativa livre quando `font_family = "system"`:
   win11 → Inter (Semibold); android → Nunito; macos → Nunito (pesos altos, arredondado); ios/ipados → Inter
   (ExtraBold no relógio). Outros valores de `font_family` (`sans`, `mono`, `serif`) continuam sobrepondo.
   Se a fonte não estiver instalada, cai para a fonte do sistema sem erro. `install.sh` verifica
   `fc-list` e, se faltar, imprime `sudo pacman -S inter-font ttf-nunito` (não instala nada sozinho).

## Não-objetivos

- Nada de logotipos, fontes ou papéis de parede proprietários (Segoe, SF, Google Sans, imagens da Apple/MS).
- Não alterar o pacote oficial `cosmic-greeter` nem `/usr/bin/*`.
- Sem widgets que exijam serviço novo (clima, notificações, calendário). Consultas novas a UPower e
  NetworkManager, que o greeter já usa, estão dentro do escopo (item 6).
- Sem reconhecimento facial/digital: ícones de impressão digital no protótipo não entram.

## Anti-objetivos (falha mesmo se "funcionar")

- Qualquer regressão na autenticação (o bug de PAM de 24/11 não pode voltar; `resolve_pam_service` fica).
- Tela de bloqueio que demora a aparecer ou deixa a área de trabalho visível antes do bloqueio (o desfoque
  nunca pode atrasar a primeira pintura: sem cache pronto, mostra a imagem sem desfoque e troca quando pronto).
- Uso de CPU contínuo com a tela parada (já houve 13 h de CPU num bloqueio): animações e timers só redesenham
  quando algo muda; timer de 1 s só quando o estilo mostra segundos.
- Texto ilegível em qualquer combinação de tema × estilo × papel de parede claro/escuro.

## Restrições

- Contrato `~/.config/cosmic-taskbar/greeter.toml`: chaves inalteradas. Valores novos `ios`/`ipados`; os antigos
  `editorial`/`sidebar` continuam aceitos como sinônimos nos dois parsers (`greeter/src/style_config.rs` e
  `src/control_panel/data.rs`); o Painel passa a gravar os novos.
- O processo de login do boot roda como usuário `cosmic-greeter` e lê o snapshot
  `/var/lib/aenes-greeter/greeter.toml` gerado por `apply-greeter.sh`.
- Faillock `deny=3`: nos testes manuais, no máximo 1 senha errada por rodada.
- Ambiente de dev: shell em sandbox Flatpak; build/print via `flatpak-spawn --host`.

## Abordagens consideradas

1. **Reescrever os 5 ramos de `screen_styled` dentro de `login_ui.rs`** (arquivo único, ~1 450 linhas hoje).
   Menos arquivos, mas `login_ui.rs` passaria de 2 000 linhas e cada estilo fica difícil de revisar.
2. **Um módulo por estilo** (`greeter/src/styles/{win11,android,macos,ios,ipados}.rs`) com uma paleta/tema
   comum (`styles/palette.rs`) e peças compartilhadas (vidro, pílula, botão redondo) em `styles/parts.rs`;
   `screen_styled` só despacha. **Escolhida** (escolha do Claude, o usuário delegou o "como"): cada estilo é
   revisável isoladamente, a paleta clara/escura fica num lugar só e a prévia do Painel pode reaproveitar as
   mesmas constantes de layout.
3. **Prévia do Painel renderizando o greeter de verdade** (biblioteca compartilhada). Fidelidade total, mas
   acopla o Painel ao crate do greeter (dependências pesadas, outra versão do libcosmic). Descartada; a prévia
   continua desenho próprio, alinhada por constantes e prints.

Desfoque: pré-processar no `apply-greeter.sh` (só boot, só imagem fixa; o bloqueio nunca teria) ×
**runtime no greeter com cache** (vale nos dois modos e para imagens rotativas). Escolhido runtime; o script
para de desfocar/escurecer a cópia (acaba o escurecimento em dobro no login).

## Decisões de detalhe (do Claude, para o usuário conferir)

- Desfoque cobre **todas** as imagens de fundo: a do `greeter.toml` e as por monitor do cosmic-bg (que chegam
  como bytes, `common.rs:172-177`). Reduzir a imagem para no máx. 1/4 da resolução do monitor antes do
  `image::imageops::fast_blur`; cache em `$XDG_CACHE_HOME/aenes-greeter/` (bloqueio) e
  `/var/lib/aenes-greeter/cache/` (boot, criado pelo script com dono `cosmic-greeter`); chave = hash dos bytes
  da imagem + raio + tamanho alvo (vale para arquivo e para bytes). `BgSource::Color` não é desfocado.
  `blur_radius = 0` é respeitado em todos os estilos, inclusive win11.
- "Vidro" (pílulas, botões redondos, widgets, cartão): tinta translúcida + borda clara de 1 px + sombra suave,
  desenhada pelo greeter sobre o fundo (já desfocado se `blur_radius > 0`). Não usa o blur do compositor
  (`common.rs:399-430` fica só para os menus).
- Legibilidade sem cartão: a paleta é escolhida pelo tema, mas o greeter mede a luminância média da região do
  texto no fundo reduzido; se o contraste do texto contra ela ficar < 4.5:1, aplica um véu suave (gradiente
  local) atrás do relógio/campo e sombra no texto. Os testes de contraste usam essa luminância medida.
- Cores tonais do Android: média ponderada da imagem reduzida, por monitor, ajustada para tom claro (campo) e
  escuro (botões); sem lib de Material You. Se `accent_color` estiver preenchido, ele vence.
- `font_weight`: `normal` = peso nativo do estilo; outros valores sobrepõem o peso do relógio (como hoje,
  `login_ui.rs:972`).
- Rotação no login com `rotate_on_lock`: escolhe pelo horário (como `rotate_interval`), sem contador em disco.
  O `lock_count` do bloqueio continua por processo.
- Segundos: só aparecem nos estilos com relógio em uma linha (win11, macos, ios, ipados); android ignora
  `show_seconds` e não liga o timer de 1 s.
- `apply-greeter.sh`: copia a imagem original (sem blur/dim); defaults do script alinhados aos do parser.
- Teste quebrado de `greeter/src/offset.rs` (renderer `()`): consertado primeiro, só no teste, para a suíte
  inteira voltar a rodar (pré-requisito de verificação).
- Scripts de teste visual aninhado (`~/Projetos/cosmic-greeter/scripts/dev/*`) trazidos para
  `greeter/scripts/dev/` apontando para o binário deste projeto.

## O que fica para quem constrói

Medidas exatas (espaçamentos, raios, tamanhos), curvas de animação de entrada, como encaixar o menu de
sessão/teclado em cada estilo, e a ordem dos widgets do iPadOS.

## Verificação

- Unitários: aliases de estilo (2 parsers), paleta clara/escura (contraste calculado ≥ 4.5:1), chave e
  validade do cache de blur, índice de rotação por tempo, escolha de usuário/sessão padrão, cartão com
  opacidade 0 = sem cartão.
- `cargo test` do greeter e do cosmic-taskbar verdes (contagem mostrada).
- Prints de cada estilo × tema (10) pelo harness aninhado e prints da prévia do Painel; aprovados pelo usuário.
- Bloqueio real: bloquear e desbloquear com a senha correta (usuário).
