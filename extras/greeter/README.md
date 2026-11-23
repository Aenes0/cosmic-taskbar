# Sistema de Login Isolado (`aenes-greeter`)

Empacota o greeter customizado (estilo Windows 11 — variante C em `~/Projetos/cosmic-greeter`) de forma **100% isolada** do pacote oficial `cosmic-greeter` do sistema (`pacman`), permitindo configuração direta pelo Painel de Controle do `cosmic-taskbar`.

## Por que isolar?

O pacote oficial `cosmic-greeter` gerencia:
- `/usr/bin/cosmic-greeter`
- `/usr/bin/cosmic-greeter-start`
- `/usr/lib/systemd/system/cosmic-greeter.service`
- `/etc/greetd/cosmic-greeter.toml`

Qualquer atualização via `pacman -Syu` sobrescreve esses caminhos. Nesta arquitetura isolada, **nenhum desses arquivos é modificado**:

| Componente | Caminho isolado | Propósito |
|---|---|---|
| Config do usuário | `~/.config/cosmic-taskbar/greeter.toml` | Editada pelo Painel de Controle ou manualmente |
| Binário customizado | `/usr/local/bin/aenes-greeter` | Compilado de `~/Projetos/cosmic-greeter` (`isolated_binary = true`) |
| Wrapper de boot | `/usr/local/bin/aenes-greeter-start` | Carrega `/var/lib/aenes-greeter/env` e sobe `cosmic-comp` |
| Aplicador root | `/usr/local/bin/apply-greeter` | Chamado via `pkexec` pelo Painel de Controle |
| Política Polkit | `/usr/share/polkit-1/actions/io.github.Aenes0.CosmicGreeterApply.policy` | Prompt descritivo no `pkexec` |
| Config `greetd` | `/etc/greetd/aenes-greeter.toml` | Aponta o `greetd` para `/usr/local/bin/aenes-greeter-start` |
| Unit `systemd` | `/etc/systemd/system/aenes-greeter.service` | Substitui `display-manager.service` sem alterar a unit oficial |
| Estado & wallpaper | `/var/lib/aenes-greeter/` | Cópia do wallpaper (com blur/dim aplicados), `env`, snapshot e backups |
| Estado `cosmic-config` | `/var/lib/cosmic-greeter/.config/cosmic/` | `last_user`, `users` (sessão), tema (`is_dark`) e relógio (`24h`/`segundos`) |

> **Nota sobre o usuário `cosmic-greeter`:** o binário do `cosmic-greeter` decide entre modo *greeter* (login) e *locker* (tela de bloqueio) checando se o usuário atual se chama `cosmic-greeter`. Por isso `/etc/greetd/aenes-greeter.toml` mantém `user = "cosmic-greeter"` e `/var/lib/aenes-greeter/` usa permissões `0755`/`0644`, garantindo leitura tanto por `cosmic-greeter` quanto por `_greetd`.

## Schema de `~/.config/cosmic-taskbar/greeter.toml`

Veja o template completo em [greeter.toml.example](greeter.toml.example):

```toml
wallpaper = ""           # caminho absoluto ou "" (usa wallpaper do usuário)
clock_24h = true         # true = 24h, false = 12h
show_seconds = false     # exibir segundos no relógio
blur_radius = 0          # 0..60 px (aplicado na cópia em /var/lib/aenes-greeter/)
dim_opacity = 0.0        # 0.0..0.85 (escurecimento sobre o wallpaper)
theme = "dark"           # "dark" | "light"
default_user = ""        # usuário pré-selecionado ou "" (último usuário)
session = "COSMIC"       # sessão pré-selecionada para default_user
isolated_binary = true   # true = /usr/local/bin/aenes-greeter, false = /usr/bin/cosmic-greeter
```

## Uso

```sh
# 1. Criar configuração inicial do usuário (ou editar pelo Painel de Controle > Tela de Login)
mkdir -p ~/.config/cosmic-taskbar
cp extras/greeter/greeter.toml.example ~/.config/cosmic-taskbar/greeter.toml

# 2. Aplicar no sistema (cria backup automático em /var/lib/aenes-greeter/backups/)
sudo ./extras/greeter/apply-greeter.sh

# 3. Verificar estado atual (sem root)
/usr/local/bin/apply-greeter --status

# 4. Forçar recompilação após editar ~/Projetos/cosmic-greeter
sudo /usr/local/bin/apply-greeter --rebuild

# 5. Reverter para o cosmic-greeter.service oficial a qualquer momento
sudo /usr/local/bin/apply-greeter --restore
```

`apply-greeter.sh` **nunca** usa `systemctl enable --now`, evitando encerrar sua sessão gráfica ativa; a nova tela de login entra em vigor no próximo logout ou reinicialização.
