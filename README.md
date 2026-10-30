# cosmic-taskbar: a barra de tarefas e o menu Iniciar do Windows 11 para o COSMIC

Um programa só, em dois applets do painel do COSMIC e um serviço:

- **Barra de tarefas**: uma fila só com os apps fixados e os abertos, com **prévia das janelas ao
  passar o mouse** (ao vivo, no estilo do macOS), a linha embaixo do app aberto, lista de atalhos
  no clique direito, contadores, progresso e atenção.
- **Menu Iniciar**: pesquisa, fixados, janelas abertas, recomendados e todos os apps (por
  categoria, grade ou lista), grupos, desinstalar apps, e o botão de energia. Abre com a tecla
  Super; Win+X abre as ferramentas.

Este repositório junta e substitui `cosmic-ext-applet-taskbar` e `cosmic-ext-applet-start`, que
estão em depreciação (o histórico dos dois está aqui).

## Prévia das janelas

Parado um instante sobre um app aberto, as janelas dele aparecem em miniatura, ao vivo (a imagem
se atualiza a cada segundo):

- uma janela: só a miniatura; duas ou mais: um painel de vidro sem borda, com a pílula do nome do
  app em cima;
- as miniaturas têm a mesma altura e a largura segue o formato de cada janela (retrato incluído);
- o ícone do app no canto da miniatura, o botão de fechar redondo no canto superior direito ao
  passar o mouse, e o **título da janela sob o mouse** na pílula, em até duas linhas (a fonte
  diminui 2 e 3 pontos antes de cortar);
- a miniatura sob o mouse cresce um pouco com mola; a prévia entra e sai com fade; tudo a 30
  quadros por segundo e parado quando nada se move;
- clicar numa miniatura vai para a janela (ou a minimiza, se já está na frente, mesmo em outro
  monitor); o botão do meio fecha.

## Menu Iniciar

O menu é um serviço (`--menu`) que fica residente, escondido entre os usos; o botão do painel e a
tecla Super só pedem a ele que apareça. Ele desenha **na CPU** (sem GPU), só aparece quando o
conteúdo está pronto e desliza como uma peça, como uma gaveta saindo do painel. Esc fecha um menu,
depois limpa a pesquisa, depois fecha tudo; clicar fora também fecha.

## Instalar

```sh
./install.sh            # gera o renderizador corrigido, compila e instala em ~/.local
./setup.sh              # põe a barra e o menu no painel e a tecla Super no menu
```

`./setup.sh --undo` devolve a bandeja de apps, o lançador e a biblioteca de apps do COSMIC.
Precisa de Rust 1.93 ou mais novo e das bibliotecas de desenvolvimento do Wayland e do xkbcommon.

Um binário só (`cosmic-taskbar`) com os modos:

| Modo | O que é |
|---|---|
| `cosmic-taskbar taskbar` | o applet da barra de tarefas (o painel o inicia) |
| `cosmic-taskbar start` | o applet com o botão Iniciar (o painel o inicia) |
| `cosmic-taskbar --menu` | o serviço do menu (Super); chamado de novo, só mostra ou esconde |
| `cosmic-taskbar --tools` | o menu de ferramentas (Win+X) |
| `cosmic-taskbar --show-desktop` | mostrar a área de trabalho |

Chamado pelos nomes antigos (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`), que o
`install.sh` deixa como links, ele assume a parte certa: atalhos e entradas do painel feitos pelos
dois programas antigos continuam valendo. Os IDs dos applets não mudaram:
`io.github.Aenes0.CosmicTaskbar` e `io.github.Aenes0.CosmicStart`.

## Como é feito

Cada parte roda num processo próprio (uma animação de uma não trava a outra, e uma queda não derruba
as outras), mas o código é um só:

- `src/bar/`: a barra e a prévia;
- `src/start/`: o menu, o botão e o serviço;
- `src/shared/`: o que as duas usam (molas e o interruptor de movimento, idioma; a captura de
  miniaturas passa para cá).

O renderizador por software (tiny-skia) leva 4 correções de desempenho
(`third_party/libcosmic-patches`: retângulo de dano e máscara de recorte mais baratos), aplicadas
por `scripts/libcosmic-patched.sh` sobre o libcosmic do `Cargo.lock`. O `install.sh` já as gera.

`COSMETICS_REDUCE_MOTION=1` ou `animation_duration_ms = 0` no cosmic-comp desligam as animações.
`scripts/measure.sh` imprime PSS, RSS, threads e descritores do applet da barra.

## Licença

GPL-3.0-only. A base da barra vem do `cosmic-app-list` do COSMIC (System76, GPL-3.0).
