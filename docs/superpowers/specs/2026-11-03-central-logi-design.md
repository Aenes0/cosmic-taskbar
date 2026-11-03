# cosmic-taskbar: o Central (painéis, bandeja, notificações, área de transferência) e o logi-osd

Data: 2026-11-03 · Repositório: `cosmic-taskbar` · Antecedente: `2026-10-30-unificacao-design.md`

## Intenção

Tudo o que o usuário mudou no painel do COSMIC vira um projeto só: a barra e o menu Iniciar (já
aqui), o Central (painel geral à direita, painel de medições à esquerda, bandeja, histórico de
notificações, área de transferência, mostrar área de trabalho) e o logi-osd (dispositivos
Logitech plugados e desplugados). Ficam de fora o Alt+Tab (`cosmic-workspaces-aenes`), o
wallpapers (`wallpapers-monitores`) e o login (`cosmic-greeter`), que dependem de pacotes do
sistema e do compositor.

## Requisitos do usuário

1. **Reaproveitamento:** o que for igual vira uma cópia só, quando der ganho real.
2. **Encapsulamento:** se uma parte travar ou cair, não trava as outras nem o sistema.
3. **Manter todas as novidades:** os painéis esquerdo e direito estão, pela primeira vez, com
   comportamento 100% (depois das correções do carregamento do histórico). Nada disso pode regredir.

## Decisões

- **Um binário, processos separados, como hoje.** Cada modo continua sendo um processo próprio,
  iniciado pelo painel ou pela sua unidade do systemd, com os mesmos limites de hoje
  (`central-panels`: `WatchdogSec=20`, `MemoryMax=400M`, `TasksMax=64`, `Nice=10`, etc.; os outros:
  `Restart=on-failure`). Nada passa a dividir processo com outra parte: `panic = "abort"` derruba
  só o processo do modo, e o systemd o relança.
- **Subcomandos:** `cosmic-taskbar taskbar|start|--menu|--tools|--show-desktop` (como hoje),
  `cosmic-taskbar central [--panels|--monitor|--tray|--clipboard|--notifications|--show-desktop]`
  e `cosmic-taskbar logi [--demo|--listar-nos]`. O `--show-desktop` do Central (um applet) e o do
  menu (uma ação) deixam de colidir por estarem em subcomandos diferentes.
- **Nomes antigos continuam valendo:** chamado como `cosmic-ext-applet-central` ou `logi-osd`
  (links instalados pelo `install.sh`), o binário assume a parte e os argumentos de antes. As
  unidades, `.desktop` e atalhos atuais funcionam durante a troca.
- **IDs e nomes D-Bus não mudam:** `io.github.Aenes0.CosmicCentral`, `.Monitor`, `.ShowDesktop`,
  `io.github.Aenes0.CosmicTray`, `io.github.Aenes0.CosmicCentral.{Panels,Clipboard,Notifications}`,
  configs `cosmic-config` e `~/.config/logi-osd/config`. Unidades `central-*` e `logi-osd`
  mantêm o nome.
- **Uma versão de libcosmic:** `5a8bd94` + cosmic-protocols `c0cff4d` (a da barra, que casa com o
  painel instalado, epoch-1.10). O Central (`d4d71fd`, 40 commits atrás) é portado para frente; o
  logi-osd não usa libcosmic.
- **O código do Central e do logi-osd entra como está** (histórico importado por subtree), em
  `src/central/` e `src/logi/`. Só muda o que a compilação obriga (caminhos `crate::`, API do
  libcosmic) e o despacho.
- **Reaproveitamento nesta etapa (sem mudar comportamento):** uma inicialização de log só em
  `shared` (hoje há 3 cópias), a detecção de idioma do `shared::locale` também no Central (que
  ganha `fr`). Molas, captura e cartão de prévia já são compartilhados entre barra e menu; o
  Central não tem equivalentes (a transição dele é outra coisa).
- O logi-osd mantém o cliente D-Bus próprio (`dbus`/libdbus) e o `udev`. Trocar por `zbus` fica
  para depois, se valer.

## Fora de escopo

- Mudar qualquer comportamento do Central ou do logi-osd.
- Processo único hospedeiro para tudo.
- Alt+Tab, wallpapers, login, cosmic-comp.

## Riscos

- **Portar o Central para `5a8bd94`:** 40 commits do libcosmic (widgets, applet, winit/wayland).
  Pode mudar algo sutil nos painéis. Mitigação: compilar primeiro, depois a lista de verificação
  na tela (abaixo), com o usuário, antes de qualquer commit de limpeza.
- **cosmic-protocols `32283d7` → `c0cff4d`:** o crate de acessibilidade do cosmic-settings
  (`ae5c750`) usa cosmic-protocols; pode não compilar com `c0cff4d`.
- **Dependências de sistema novas:** `libudev` e `libdbus-1` (pkg-config) para o logi-osd.
- **Regra udev do logi-osd** (`/etc/udev/rules.d/70-logi-osd.rules`): já instalada; o
  `install.sh` não a reinstala (pede sudo).
- Ao trocar o binário, os serviços antigos continuam vivos com o arquivo apagado até serem
  reiniciados.

## Verificação na tela (com o usuário, antes de arquivar os repositórios)

Painel direito abre e fecha; histórico de notificações carrega certo; painel de medições à esquerda;
bandeja e seus menus; área de transferência (copiar, histórico); mostrar área de trabalho;
super-economia; um dispositivo Logitech desplugado e plugado gera o aviso; barra e menu Iniciar
iguais. Memória e threads por processo medidas antes e depois.
