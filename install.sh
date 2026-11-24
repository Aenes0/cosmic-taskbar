#!/bin/sh
# Build and install for the current user (default) or under another prefix:
#   ./install.sh                      -> ~/.local (includes in-session aenes-greeter)
#   ./install.sh --with-boot-greeter  -> also installs/enables boot login screen (sudo)
#   ./install.sh --with-power         -> also installs/enables economia-energia.service (sudo)
#   ./install.sh --with-all           -> enables both --with-boot-greeter and --with-power
#   sudo ./install.sh /usr            -> system wide
#
# One program, cosmic-taskbar, for every part; each part still runs in a
# process of its own, from the panel or from its systemd unit, so one that
# hangs or crashes takes nothing else with it (the units keep their limits
# and the panels' watchdog).
set -e
cd "$(dirname "$0")"
prefix=""
with_boot_greeter=0
with_power=0
for arg in "$@"; do
    case "$arg" in
        --with-boot-greeter) with_boot_greeter=1 ;;
        --with-power) with_power=1 ;;
        --with-all)
            with_boot_greeter=1
            with_power=1
            ;;
        -h|--help)
            echo "Uso: ./install.sh [PREFIX] [--with-boot-greeter] [--with-power] [--with-all]"
            exit 0
            ;;
        *)
            if [ -z "$prefix" ]; then
                prefix="$arg"
            else
                echo "Argumento desconhecido: $arg" >&2
                exit 1
            fi
            ;;
    esac
done
prefix="${prefix:-$HOME/.local}"
bin=cosmic-taskbar
bindir="$prefix/bin"

cargo build --release
install -Dm0755 "target/release/$bin" "$bindir/$bin"

# Greeter & Lockscreen customizado (aenes-greeter), absorvido em greeter/.
# Instalado em ~/.local/bin/aenes-greeter com symlink cosmic-greeter para que
# o lockscreen da sessão use nossa versão sem tocar em /usr/bin/cosmic-greeter.
if [ -f "greeter/Cargo.toml" ]; then
    cargo build --release --manifest-path greeter/Cargo.toml --bin cosmic-greeter
    greeter_target="${CARGO_TARGET_DIR:-greeter/target}"
    install -Dm0755 "$greeter_target/release/cosmic-greeter" "$bindir/aenes-greeter"
    ln -sf "aenes-greeter" "$bindir/cosmic-greeter"
fi

# The programs this one replaces were called by these names, by shortcuts,
# panel entries and units they set up: the old names stay as links, and the
# program takes the part its name says, with the arguments it always had.
for old in cosmic-ext-applet-taskbar cosmic-ext-applet-start cosmic-ext-applet-central logi-osd; do
    ln -sf "$bin" "$bindir/$old"
done

apps="$prefix/share/applications"
mkdir -p "$apps"
# Taskbar, Start menu, workspaces' button, and the Control Panel app.
for id in io.github.Aenes0.CosmicTaskbar io.github.Aenes0.CosmicStart io.github.Aenes0.CosmicWorkspacesButton io.github.Aenes0.CosmicControlPanel; do
    sed "s|@bindir@|$bindir|" "data/$id.desktop.in" > "$apps/$id.desktop"
done
# The Central's applets: Corner (integrated background apps + system bar icons
# + Central + clock + show desktop), Monitor, and standalone entries. Custom
# standalone Tray is omitted since background apps are integrated into Corner.
central=io.github.Aenes0.CosmicCentral
for id in "$central" "$central.ShowDesktop" "$central.Monitor" "$central.Corner"; do
    sed "s|@bindir@|$bindir|" "data/central/$id.desktop.in" > "$apps/$id.desktop"
done
rm -f "$apps/io.github.Aenes0.CosmicTray.desktop"
install -Dm0644 "data/central/$central.metainfo.xml" "$prefix/share/metainfo/$central.metainfo.xml"
install -Dm0644 "data/central/icons/scalable/apps/$central.svg" "$prefix/share/icons/hicolor/scalable/apps/$central.svg"

# The Central's services (clipboard, notifications' history, the panels'
# host), as user units also activated over D-Bus.
units="$prefix/share/systemd/user"
dbus="$prefix/share/dbus-1/services"
mkdir -p "$units" "$dbus"
for unit in central-clipboard central-notifications central-panels; do
    sed "s|@bindir@|$bindir|" "data/central/$unit.service.in" > "$units/$unit.service"
done
for name in Clipboard Notifications Panels; do
    sed "s|@bindir@|$bindir|" "data/central/$central.$name.service.in" > "$dbus/$central.$name.service"
done
rm -f "$dbus/org.freedesktop.Notifications.service"
# Isolated Alt+Tab / Super+Tab window switcher (`io.github.Aenes0.CosmicSwitcher`).
sed "s|@bindir@|$bindir|" "data/switcher/cosmic-switcher.service.in" > "$units/cosmic-switcher.service"
sed "s|@bindir@|$bindir|" "data/switcher/io.github.Aenes0.CosmicSwitcher.service.in" > "$dbus/io.github.Aenes0.CosmicSwitcher.service"

# The Logitech devices' service: for the user, where it always had its unit
# (in ~/.config, which wins over ~/.local/share); under a system prefix, with
# the Central's units (root's ~/.config would leave the user without it).
if [ "$prefix" = "$HOME/.local" ]; then
    logi_unit="$HOME/.config/systemd/user/logi-osd.service"
else
    logi_unit="$units/logi-osd.service"
fi
mkdir -p "$(dirname "$logi_unit")"
sed "s|@bindir@|$bindir|" data/logi/logi-osd.service > "$logi_unit"
install -Dm0644 data/logi/config.exemplo "$prefix/share/logi-osd/config.exemplo"
if [ ! -e /etc/udev/rules.d/70-logi-osd.rules ] && [ ! -e /usr/lib/udev/rules.d/42-logitech-unify-permissions.rules ]; then
    echo "The Logitech service needs /dev/hidraw* access; as root, once:"
    echo "  sudo install -Dm0644 data/logi/70-logi-osd.rules /etc/udev/rules.d/70-logi-osd.rules && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw"
fi

# Utilitários visuais extras (print-recorte, print-preview, fix-brave-pwa-icons,
# wallpapers-monitores, reiniciar-painel) e suas units de usuário.
for tool in extras/bin/*; do
    [ -f "$tool" ] || continue
    install -Dm0755 "$tool" "$bindir/$(basename "$tool")"
done
if [ "$prefix" = "$HOME/.local" ]; then
    user_units="$HOME/.config/systemd/user"
    mkdir -p "$user_units"
    for u in extras/systemd/*; do
        [ -f "$u" ] || continue
        install -Dm0644 "$u" "$user_units/$(basename "$u")"
    done
fi

# Only a plain ~/.local install starts the services here; system prefixes
# leave that to the packager. Restarting the panels' host closes an open
# panel, nothing else: the bar is not touched.
if [ "$prefix" = "$HOME/.local" ]; then
    # Everything is in place by now: without the user's systemd (no session
    # bus, over ssh) the services just start at the next login.
    if ! systemctl --user daemon-reload; then
        echo "Installed, but the user's systemd did not answer: the services start at the next login."
        exit 0
    fi
    for unit in central-clipboard central-notifications central-panels cosmic-switcher logi-osd; do
        systemctl --user enable "$unit.service" >/dev/null 2>&1 || true
        systemctl --user restart "$unit.service" || echo "could not restart $unit now; it starts at the next login"
    done
    # Restart resident panel-spawned hosts so they pick up the freshly installed binary.
    pkill -f "cosmic-taskbar --(menu|tools)$" >/dev/null 2>&1 || true
    pkill -f "cosmic-taskbar --taskbar-popups$" >/dev/null 2>&1 || true
    for path_unit in fix-brave-pwa-icons wallpapers-monitores; do
        systemctl --user enable --now "$path_unit.path" >/dev/null 2>&1 || true
    done
    systemctl --user enable --now cosmic-taskbar-update.timer >/dev/null 2>&1 || true
    mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/cosmic-taskbar"
    printf '%s\n' "$PWD" > "${XDG_CONFIG_HOME:-$HOME/.config}/cosmic-taskbar/repo_path"
fi
if [ "$with_boot_greeter" -eq 1 ] && [ -x "extras/greeter/apply-greeter.sh" ]; then
    echo "Applying isolated boot login screen (aenes-greeter)..."
    if [ "$(id -u)" -eq 0 ]; then
        ./extras/greeter/apply-greeter.sh
    else
        sudo ./extras/greeter/apply-greeter.sh --user "${USER:-$(id -un)}"
    fi
fi

if [ "$with_power" -eq 1 ] && [ -x "extras/power/install-economia-energia.sh" ]; then
    echo "Installing economia-energia.service..."
    if [ "$(id -u)" -eq 0 ]; then
        ./extras/power/install-economia-energia.sh
    else
        sudo ./extras/power/install-economia-energia.sh
    fi
fi

echo "Installed. Run ./setup.sh to use the taskbar and the Start menu in the panel."
if [ "$with_boot_greeter" -eq 0 ] && [ "$prefix" = "$HOME/.local" ] && ! systemctl is-enabled --quiet aenes-greeter.service 2>/dev/null; then
    echo "Tip: run ./install.sh --with-boot-greeter (or --with-all) to also enable the custom login screen at boot."
fi
if [ "$with_power" -eq 0 ] && [ "$prefix" = "$HOME/.local" ] && ! systemctl is-enabled --quiet economia-energia.service 2>/dev/null; then
    echo "Tip: run ./install.sh --with-power (or --with-all) to also install economia-energia.service."
fi
