# cosmic-taskbar: o painel do COSMIC como no Windows 11 e no macOS

Um programa só para tudo o que o painel mostra deste projeto: a barra de tarefas, o menu
Iniciar, o Central (configurações rápidas e os painéis laterais), o painel de medições, a
bandeja, o histórico de notificações, a área de transferência e os avisos dos dispositivos
Logitech. Cada parte roda num processo próprio, iniciado pelo painel ou pela sua unidade do
systemd: se uma travar ou cair, não leva as outras junto.

- **Barra de tarefas**: uma fila só com os apps fixados e os abertos, com **prévia das janelas ao
  passar o mouse** (ao vivo, no estilo do macOS), a linha embaixo do app aberto, lista de atalhos
  no clique direito, contadores, progresso e atenção.
- **Menu Iniciar**: pesquisa, fixados, janelas abertas, recomendados e todos os apps (por
  categoria, grade ou lista), grupos, desinstalar apps, e o botão de energia. Abre com a tecla
  Super; Win+X abre as ferramentas.

Este repositório junta e substitui `cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`,
`cosmic-ext-applet-central` (com a bandeja, antes `cosmic-ext-applet-tray`) e `logi-osd`, todos
depreciados e arquivados; o histórico deles está aqui.

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

## Central: configurações rápidas e os painéis laterais

O Central é o painel da direita no estilo do Windows 11, do Android e da Central de Controle do
macOS: privacidade (câmera e microfone em uso), notificações, blocos (Wi-Fi, Bluetooth com a
bateria dos dispositivos, Energia, Não perturbe, Tema escuro, Modo avião, VPN e outros), volume e
brilho, o que está tocando, calendário, e páginas de Wi-Fi, VPN, Bluetooth, Energia e Som.
O **painel de medições**, à esquerda, mostra a energia gasta, CPU, GPU, memória, discos, rede,
sensores (temperaturas e ventoinhas), processos, o sistema e o clima; na barra, o que você escolher.
Os dois painéis rodam num só processo hospedeiro (`central-panels`), com watchdog: uma interface
travada é reiniciada em 20 s, sem mexer na barra.

- **Bandeja**: os ícones dos apps em segundo plano (StatusNotifierItem) e os menus deles, ao lado
  do Central.
- **Histórico de notificações**: um serviço (`central-notifications`) é o único dono do histórico;
  as notificações chegam pelo daemon do COSMIC.
- **Área de transferência**: um serviço (`central-clipboard`) guarda o histórico do que foi copiado.
- **Mostrar área de trabalho**: um botão próprio na ponta do painel.

## Dispositivos Logitech

Um serviço (`logi-osd`) lê teclados e mouses Logitech por HID++ e avisa, pelas notificações, o
que acontece com eles: conectado, desconectado, bateria baixa, luz do teclado, Caps Lock e
outras teclas de estado. A tecla Emoji pode rodar um comando (`~/.config/logi-osd/config`).
O acesso a `/dev/hidraw*` vem de uma regra udev (`data/logi/70-logi-osd.rules`), instalada uma vez
com sudo, ou da regra do Solaar.

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
| `cosmic-taskbar --show-desktop` | mostrar a área de trabalho (atalho do Win+X) |
| `cosmic-taskbar central` | o applet do Central (o painel o inicia) |
| `cosmic-taskbar central --monitor` | o applet do painel de medições |
| `cosmic-taskbar central --tray` | o applet da bandeja |
| `cosmic-taskbar central --show-desktop` | o applet do botão de mostrar a área de trabalho |
| `cosmic-taskbar central --panels` | o hospedeiro dos dois painéis laterais (unidade `central-panels`) |
| `cosmic-taskbar central --notifications` | o histórico de notificações (unidade `central-notifications`) |
| `cosmic-taskbar central --clipboard` | a área de transferência (unidade `central-clipboard`) |
| `cosmic-taskbar logi` | os dispositivos Logitech (unidade `logi-osd`; `--demo` e `--listar-nos` para testar) |

Chamado pelos nomes antigos (`cosmic-ext-applet-taskbar`, `cosmic-ext-applet-start`,
`cosmic-ext-applet-central`, `logi-osd`), que o `install.sh` deixa como links, ele assume a parte
certa, com os argumentos de antes: atalhos, entradas do painel e unidades antigas continuam
valendo. Os IDs dos applets, os nomes no D-Bus, as configurações e os nomes das unidades não
mudaram.

## Como é feito

Cada parte roda num processo próprio (uma animação de uma não trava a outra, e uma queda não derruba
as outras), mas o código é um só:

- `src/bar/`: a barra, a prévia, o zoom e o salto;
- `src/start/`: o menu, o botão e o serviço;
- `src/central/`: o Central, os painéis laterais, a bandeja, as notificações e a área de
  transferência;
- `src/logi/`: os dispositivos Logitech (sem libcosmic, num laço próprio);
- `src/shared/`: o que mais de uma parte usa: molas e o interruptor de movimento, idioma, a
  inicialização do log, a captura de miniaturas e o cartão de prévia.

As unidades do systemd mantêm os limites de cada parte (o `central-panels` tem watchdog de 20 s,
teto de memória de 400 MB e prioridade baixa; as outras reiniciam sozinhas se caírem). Com
`panic = "abort"`, uma falha encerra só o processo daquela parte.

O renderizador por software (tiny-skia) leva 4 correções de desempenho
(`third_party/libcosmic-patches`: retângulo de dano e máscara de recorte mais baratos), aplicadas
por `scripts/libcosmic-patched.sh` sobre o libcosmic do `Cargo.lock`. O `install.sh` já as gera.

`COSMETICS_REDUCE_MOTION=1` ou `animation_duration_ms = 0` no cosmic-comp desligam as animações.
`scripts/measure.sh` imprime PSS, RSS, threads e descritores do applet da barra.

## Licença

GPL-3.0-only. A base da barra vem do `cosmic-app-list` do COSMIC (System76, GPL-3.0); o Central
começou como fork das [Configurações rápidas](https://github.com/DrakRosmann/cosmic-ext-applet-quick-settings)
de DrakRosmann (GPL-3.0).
