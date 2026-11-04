# Um processo para o canto direito da barra: design

> 03/11/2026. Aprovação pendente. Ninguém commita sem pedido; trabalho direto em `main`.

## Intenção

Cada tela tem hoje 7 processos do `cosmic-taskbar` no painel. O usuário quer **tudo visível nas
três telas, o tempo todo** (sem painel mínimo nos secundários e sem nada que siga o mouse) e
**menos processos**. O cosmic-panel abre um processo por applet por tela, e esse modelo não muda.
O que muda é quantos applets o painel vê: os três do canto direito viram **um applet só**.

## O que se funde (decisões do usuário)

- **Juntam-se:** bandeja (`central --tray`), Central/relógio (`central`) e mostrar área de trabalho
  (`central --show-desktop`), na ordem de hoje: bandeja, Central, mostrar área.
- **Fica como está:** o monitor de uso (`central --monitor`), na asa esquerda, processo próprio.
- **Não muda:** Iniciar, barra, áreas de trabalho e o hospedeiro `--panels`.

## Objetivos

- Por tela, 3 processos viram 1. No total: −6 processos.
- Medido antes (03/11, PSS por tela): `central` 8,7 MB/12 threads, `--tray` 7,2 MB/6,
  `--show-desktop` 6,7 MB/6 = **22,6 MB e 24 threads por tela, 68 MB nas três**. Esperado depois:
  ~10–12 MB por tela, **~35 MB a menos no total** (estimativa; medir no fim).
- Visual e comportamento idênticos: mesmos ícones, mesmos espaçamentos, mesma ordem; o clique no
  relógio abre o painel do Central; o menu de um ícone da bandeja e a caixa do ^ abrem no mesmo
  lugar; arrastar ícones na bandeja funciona; mostrar área de trabalho funciona.

## Não-objetivos

- Não fundir o monitor de uso, o Iniciar, a barra nem as áreas de trabalho.
- Não ter um processo para as três telas (exigiria desenhar a barra fora do cosmic-panel).
- Não mexer no zoom nem nas prévias da barra.

## Anti-objetivos (falha mesmo funcionando)

- Um menu da bandeja abrindo deslocado, ou o arrastar de ícones mirando o lugar errado.
- O relógio ou a bandeja "pulando" de largura quando ícones entram ou saem.
- Perder o caminho de volta: o `setup.sh --undo` tem de restaurar os 3 applets separados.

## Restrições e o que já existe

- **Molde pronto:** `src/central/panels/mod.rs` já embrulha `App` e `Monitor` num processo
  (`enum Message { Central(..), Monitor(..) }`, `lift(task, wrap)` em `:151`, delegação de
  `update`/`view_window`). O applet fundido segue esse padrão.
- Cada applet de hoje se envolve em `applet.autosize_window(..)` no próprio `view`
  (`app.rs:2166`, `show_desktop.rs:189`, `tray/components/view.rs:77`). Fundidos, só o invólucro
  faz o autosize; cada um passa a expor o conteúdo sem o autosize.
- A bandeja redimensiona a janela principal sozinha (`window::resize` em
  `tray/components/app.rs:181-196`) e posiciona os menus por `slot × button_len` a partir da
  borda da janela (`:161-178`). Com a bandeja **primeira** na ordem, o deslocamento dela dentro da
  janela é zero na horizontal; o que muda é o tamanho: a janela fundida mede bandeja + Central +
  mostrar área, não só a bandeja.
- Os três modos continuam rodando sozinhos (os `.desktop` e as flags ficam), para o `--undo` e
  para depurar.
- A bandeja guarda a configuração no `APP_ID` dela (`io.github.Aenes0.CosmicTray`), o Central no
  dele: continuam valendo dentro do applet fundido.
- **Cliques e passagem do mouse são da janela inteira hoje** (o ponto mais delicado): o Central
  abre o painel com qualquer clique esquerdo na janela (`app.rs:1898-1901` → `PanelPress`,
  `:965-973`, que ignora o clique que acabou de fechar o painel, < 250 ms), e o mostrar área usa
  `ButtonPressed`/`CursorEntered`/`CursorLeft` da janela para clicar e espiar
  (`show_desktop.rs:137-149`, "a janela é o botão"). Na janela fundida, um clique na bandeja
  abriria o Central e esconderia as janelas. **Cada um passa a receber clique e passagem só na sua
  área** (ex.: `mouse_area` com `on_press`, que dispara no aperto como hoje, e `on_enter`/
  `on_exit`), mantendo a regra dos 250 ms do Central. Os modos sozinhos continuam como estão.
- **Capacidade da bandeja:** ela conta quantos ícones cabem pelo `suggested_bounds` do applet
  (`tray/components/app.rs:136-145`), que o painel só manda quando o aperta (transbordando).
  Fundida, a bandeja recebe esse limite já descontada a largura do Central e do mostrar área.
- `show_desktop` abre uma conexão Wayland própria numa thread (`show_desktop.rs:137-164`) e a
  bandeja tem um `Subscription` de id fixo: uma cópia de cada por processo, como hoje.
- Instalar reinicia os 21 applets do painel uma vez; só com o OK do usuário.

## Escolhas minhas (o usuário pode trocar)

- Nome do applet: `io.github.Aenes0.CosmicCentral.Corner` ("Canto"), flag `central --corner`,
  `.desktop` novo em `data/central/`, módulo novo `src/central/corner.rs`.
- O `setup.sh` troca, em `plugins_wings`, os 3 ids pelo novo (no lugar onde estava a bandeja);
  o `--undo` faz o inverso. Se o usuário tiver tirado algum dos 3 do painel, o `setup.sh` não
  inventa o canto: avisa e não mexe.
- O `.desktop` do canto leva `X-HostWaylandDisplay=true` (o Central e o mostrar área precisam;
  para a bandeja é inofensivo), `X-CosmicHoverPopup=Auto`, `X-OverflowPriority=99` e
  `X-OverflowMinSize=2` (os da bandeja). Com o painel apertado, o bloco encolhe e quem cede é a
  bandeja, que recolhe ícones no ^ como hoje; o relógio e o mostrar área ficam.
- O `setup.sh` hoje não mexe nesses 3 ids: a troca e o `--undo` são código novo.

## Deixado ao construtor

- Como cada filho expõe o conteúdo sem autosize (método `content()` ou parâmetro), e como a
  bandeja passa a informar a largura em vez de redimensionar a janela.
- Como o invólucro mede a largura do Central (relógio variável) para descontar da bandeja.
- A ordem e o formato dos testes; o que testar com teste de unidade (largura somada, deslocamento
  dos menus, ida e volta do `setup.sh`) e o que conferir na tela com o usuário.

## Verificação

- `flatpak-spawn --host cargo test` verde com contagem; `cargo build --release`.
- Instalado (com OK): `ps -eo args | grep cosmic-taskbar` mostra 3 `central --corner` e nenhum
  `central`, `--tray` ou `--show-desktop` sozinho; PSS somado medido e comparado com os 68 MB.
- Na tela, com o usuário: menus da bandeja, caixa do ^, arrastar, relógio abrindo o Central,
  mostrar área de trabalho, nas três telas.
