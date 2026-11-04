# Bandeja reconstruída por dentro: design

> 03/11/2026. Aprovação pendente. Ninguém commita sem pedido.

## Intenção

A bandeja (`src/central/tray/`, fork do `cosmic-applet-status-area`) "sempre foi instável". Visual
**idêntico**; muda só o funcionamento. Vale para os dois jeitos de rodar: dentro do Canto
(`central --corner`) e sozinha (`central --tray`).

## O que o usuário vive hoje (palavras dele)

- Clicar no ^ "é sempre uma loteria": às vezes abre, às vezes não; muitas vezes só funciona depois
  de abrir o painel da direita e voltar.
- Clique direito num ícone **da caixa** abre o menu do item, mas a caixa fecha; deveria ficar.
- Arrastar ícones nunca foi usado a fundo; **tem que existir e funcionar**.

## Causas confirmadas no código

- O libcosmic (pin 5a8bd94), quando já há popup com grab ou um sendo destruído, guarda o pedido
  novo num slot único e tenta 5 × 30 ms; esgotado, **desiste sem avisar**
  (`iced/winit/.../event_loop/state.rs:1395-1440`).
- Destruir um id que não existe **sempre esvazia o slot pendente**, cancelando o pedido de outro
  popup que estivesse esperando (`state.rs:1883`); o aviso "Destroyed popup does not exist" sai
  quando o pendente era outro (ou não havia nenhum).
- A bandeja marca `overflow_popup`/`popup`/`open_menu` como abertos **no pedido** (dentro da
  closure do `app_popup`, `tray/components/app.rs:282,289,320`), não quando o popup existe. Com o
  pedido perdido, o estado diz "aberto"; o próximo clique "fecha" o fantasma e só o seguinte abre.
- O menu fecha a caixa por regra antiga ("dois popups com grab confundem o compositor",
  `app.rs:264-266`, aplicada em `:278` e `:418`), e o `Hovered` com menu aberto reabre outro menu
  fechando a caixa (`:563-569`).
- Sinal disponível para confirmar: na primeira configuração de um popup o iced emite
  `window::Event::Opened` com o id do popup (`sctk_event.rs:1144-1150`); o fim vem por
  `on_close_requested`/`PopupEvent::Done`.

## Objetivos

1. **Popups que só acreditam no que existe.** Cada popup (caixa, menu) passa por estados
   explícitos: *pedido* → *aberto* (ao receber `Opened` do id) → *fechando* → fechado (ao receber
   o fechamento). Regras:
   - um clique decide abrir/fechar pelo estado **confirmado**, nunca pelo pedido;
   - nunca destruir um id que não esteja *aberto* (nada de destroy às cegas);
   - nunca pedir um popup novo enquanto outro está *fechando*: o próximo espera o fechamento;
   - um *pedido* sem `Opened` em ~300 ms é dado como perdido e volta a fechado, para o clique
     seguinte abrir.
2. **Menu filho da caixa.** Clique direito num ícone da caixa abre o menu com pai = caixa,
   ancorado no ícone; a caixa fica aberta por baixo (`close_with_children = false`); fechar o
   menu devolve à caixa; fechar a caixa fecha o menu. Passar o mouse entre ícones da caixa não a
   fecha. Menu de ícone da **barra** continua com pai = janela do applet.
3. **Arrastar funcionando e testado:** barra → caixa (no ^), caixa → barra (numa posição),
   reordenar na barra; a escolha persiste na configuração como hoje.
4. **Correções do oficial** (pop-os/cosmic-applets, `cosmic-applet-status-area`), conferindo
   uma a uma se já temos: `c8104a98` (layout do menu sob demanda; antes todos os itens pediam
   `GetLayout` e apps como o VLC travavam o D-Bus), `a502720a` (atualizar o menu em
   PropertyUpdate), `6e99ad42` (item duplicado no watcher), `ebe18a17` (botão desabilitado
   quando `enabled=false`), `bd0d1804` (`break` no lugar de `unwrap` em `unique_names.rs`, que
   aqui não existe: achar o equivalente no nosso watcher),
   `fddf3d56` (repassar Scroll ao item), `63292ab7` (icon-data do menu em 14 px), `d73ef7dc`
   (ícones no theme path do app).

   Pela revisão, `c8104a98` (`is_open` em `status_menu.rs:178`), `ebe18a17` (`enabled()` em
   `status_menu.rs:260`) e `fddf3d56` (Scroll em `app.rs:571,732`) parecem já presentes:
   confirmar e seguir.

## Não-objetivos

- Nenhuma mudança visual (grade de 5 por linha, ícones, menus, ^, espaçamentos).
- Não reescrever a conversa D-Bus/StatusNotifierItem além das correções do oficial.
- Não mexer no Central, no mostrar área nem no monitor de uso.

## Anti-objetivos

- O ^ ou um menu que "às vezes" não abre, por qualquer caminho.
- A barra inteira sem resposta depois de usar a bandeja (o quadro de 28/10 e 03/11).
- Um log `Destroyed popup does not exist` ou `ParentMissing` vindo da bandeja.

## Restrições

- libcosmic no pin 5a8bd94 (não atualizar).
- No Canto, os popups da bandeja passam por `corner::tray_surface` (o reembrulho do `AppPopup`):
  o desenho novo tem de continuar passando por ali ou usar um caminho que não dependa do tipo.
- Os testes de unidade cobrem a máquina de estados sem Wayland (eventos simulados: pedido,
  `Opened`, fechamento, tempo); o teste real é na tela, logo depois de um `reiniciar-painel`.

## Decisões do usuário (03/11)

- Com a caixa e o menu de um ícone dela abertos, **passar o mouse** sobre outro ícone da caixa
  **não faz nada**: só o clique direito troca o menu (como no Windows 11).
- **Clicar fora** de tudo fecha **menu e caixa juntos**; **Esc** fecha só o menu.
- Com a caixa aberta, clique direito num ícone **da barra** abre o menu dele e **fecha a caixa**.

## Escolhas minhas (o usuário pode trocar)

- Um popup *dado como perdido* que receba `Opened` depois: é adotado se nada outro foi pedido
  desde então; senão é fechado.
- O id que a bandeja guarda é o mesmo que chega no `Opened` também no Canto (a closure do
  `app_popup` gera e guarda o id e roda sobre a bandeja via `corner::tray_surface`); o construtor
  confirma com um teste.
- O estado atual do arrastar é desconhecido (no Canto o arrasto "não começou", mas o painel
  estava no quadro de travamento): o construtor mede primeiro, com a bandeja sozinha e no Canto,
  depois de `reiniciar-painel`.

## Deixado ao construtor

- Onde fica a máquina de estados (módulo próprio sugerido: `tray/components/popups.rs`), nomes e
  tipos.
- O valor exato do tempo de desistência (~300 ms) e se o `X-CosmicHoverPopup=Auto` precisa de
  tratamento.
- A ordem: máquina de estados primeiro, depois menu filho, arrastar e correções do oficial.

## Verificação

- `cargo test` verde com contagem, com testes da máquina de estados para: clique rápido duplo,
  pedido perdido (sem `Opened`), fechamento pelo compositor, menu sobre a caixa, hover entre
  ícones da caixa.
- Instalado (com OK) e depois de `reiniciar-painel`: 20 cliques seguidos no ^ alternam
  abrir/fechar sem falha; clique direito na caixa mantém a caixa; arrastar nos 3 sentidos; log
  sem `Destroyed popup does not exist`/`ParentMissing` da bandeja.
