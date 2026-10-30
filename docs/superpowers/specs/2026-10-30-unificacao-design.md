# cosmic-taskbar: barra de tarefas e menu Iniciar num projeto só

Data: 2026-10-30 · Repositório novo: `cosmic-taskbar` (local `~/Projetos/cosmic-taskbar`) · Branch: `main` (escolha do usuário; Claude não commita sem pedido explícito)

## Intenção

Hoje a barra (`cosmic-ext-applet-taskbar`) e o menu Iniciar (`cosmic-ext-applet-start`) são dois
projetos, dois binários e duas bases de código com muita coisa repetida (lista de janelas,
captura de miniaturas, prévia, ícones, textos, estilo, animação). O usuário quer **um projeto só,
quase um app só**: um repositório, um binário, e o máximo de código, visual (CSS/tokens) e caches
em comum, para ganhar desempenho e economia, e para cada correção valer nos dois lugares.

## Decisões do usuário

- Repositório **novo, `cosmic-taskbar`**, importando os dois com histórico. Os repositórios
  antigos (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`) entram em **depreciação**
  (aviso no README e arquivar), para não confundir.
- **Um binário, processos separados** (modos: barra, botão Iniciar, serviço do menu). Não é um
  processo único: uma animação de um não pode travar o outro, e uma queda não derruba tudo.
- Trabalho direto em `main` do repositório novo.
- Reaproveitar visuais, CSS e tudo que trouxer desempenho e economia.

## Objetivos

1. Um repositório, um `Cargo.lock`, um binário `cosmic-taskbar` com os modos: `taskbar` (applet da
   barra), `start` (applet do botão Iniciar), `--menu`, `--tools`, `--show-desktop`.
2. Código compartilhado, uma vez só: janelas (toplevels), captura de miniaturas, ícones
   (cache e pré-rasterização), textos pt/en, animação (molas), tokens de estilo, cartão de prévia.
3. Os dois IDs de applet continuam: `io.github.Aenes0.CosmicTaskbar` e
   `io.github.Aenes0.CosmicStart`. O painel do usuário não muda de configuração.
4. Medir antes e depois (memória, threads, descritores, CPU em interação) e não piorar.
5. `cargo test` verde de ponta a ponta.

## Não objetivos

- Não vira um processo único hospedeiro (pode vir depois, com medição, se valer).
- Não muda o comportamento do menu nem da barra: a fusão é de código, não de recursos.
- Não mexe no Central, na bandeja, no Alt+Tab nem no cosmic-comp.

## Anti-objetivos (seria falha mesmo que compile)

- Perder o histórico git dos dois projetos.
- Piorar consumo parado (hoje barra 18,7 MB PSS por processo, 36 threads; menu: botão ~10 MB e
  serviço ~34 MB).
- Quebrar o painel: IDs de applet, `.desktop` e atalhos (Super, Win+X) têm de continuar valendo.
- Janela da prévia ou menu piscando, ou animação mais pesada que a de hoje.

## Constraints e fatos (relatados por agentes de leitura, ainda não conferidos por compilação)

- libcosmic: barra em `5a8bd94`, menu em `d4d71fd` (ambos 1.0.0 / iced 0.14). Diferem em ~79
  arquivos (widgets, winit/wayland, cosmic-config, `src/applet`, `src/app`); nenhum em
  `iced/tiny_skia`. cosmic-protocols: barra `c0cff4d`, menu `32283d7`.
- O painel instalado é epoch-1.10.0; a barra já usa os pins que casam com ele.
- O menu usa 4 patches no renderizador por software (`iced_tiny_skia`: bbox de dano, máscara de
  recorte, desvincular do workspace, aviso morto), só otimizações, via `[patch]` para um
  `libcosmic-patched` local gerado por script.
- O menu liga o recurso `wgpu`; a barra não. O botão do menu usa renderizador por software.
- `cargo test` do menu não compila hoje: `src/menu/load_icons.rs:54` importa `load::tests::entry`
  privada (E0603).
- Fixados: a barra lê e escreve `favorites` em `com.system76.CosmicAppList`; o menu só semeia a
  partir dele e depois usa chave própria. Não unificar isso nesta etapa.

## Decisões minhas (marcadas, o usuário pode trocar)

- **Um só crate** com árvore de módulos (`src/shared/`, `src/bar/`, `src/start/`) e um binário.
  Workspace de vários crates só se o tempo de compilação pedir.
- **Base de libcosmic: `5a8bd94` + cosmic-protocols `c0cff4d`** (a da barra, que casa com o painel
  instalado). O menu é portado para ela; os 4 patches do renderizador são reaplicados sobre ela.
- **Captura de miniaturas: a do menu** (`capture.rs` + worker, refatorada e testada) vira a única;
  a barra passa a usá-la no lugar da sua.
- **Ícones: o cache e a pré-rasterização do menu** passam a servir à barra também.
- **Fixados** ficam como estão (cada um com sua chave) até decisão separada.
- Textos: um módulo, duas structs (`Texts` do menu e a da barra).
- Estilo: tokens comuns (raios, sombra, vidro, alfa) num módulo `shared::style`; as cores do menu
  (paleta estilo Windows 11) ficam como estão até a barra e o menu usarem o mesmo cartão de prévia.
- Instalação: um `install.sh` e um `setup.sh`; os `.desktop` dos dois IDs apontam para
  `cosmic-taskbar taskbar` e `cosmic-taskbar start`. Atalhos usam `cosmic-taskbar --menu` e `--tools`.

## O que fica a cargo de quem constrói

Nomes internos dos módulos, ordem exata das extrações, como organizar os testes, e os detalhes da
reaplicação dos patches do renderizador.

## Riscos

- Portar o menu para o libcosmic da barra pode quebrar widgets/applet (79 arquivos diferentes).
  Mitigação: compilar o menu sozinho nessa base antes de qualquer extração.
- Unificar recursos do Cargo (`wgpu`) pode puxar a GPU para a barra. Mitigação: manter o recurso
  desligado e confirmar com `ldd`/maps que nenhuma lib de GPU é carregada.
- Trocar o binário do serviço `--menu` exige reiniciá-lo (o antigo fica vivo com o arquivo apagado).
- Depreciar e arquivar os repositórios antigos é ação pública no GitHub: só com confirmação do usuário.
