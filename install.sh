#!/bin/sh
# Build and install for the current user (default) or under another prefix:
#   ./install.sh            -> ~/.local
#   sudo ./install.sh /usr  -> system wide
#
# One program, cosmic-taskbar, for every part; each part still runs in a
# process of its own, from the panel or from its systemd unit, so one that
# hangs or crashes takes nothing else with it (the units keep their limits
# and the panels' watchdog).
set -e
cd "$(dirname "$0")"
prefix="${1:-$HOME/.local}"
bin=cosmic-taskbar
bindir="$prefix/bin"

cargo build --release
install -Dm0755 "target/release/$bin" "$bindir/$bin"

# The programs this one replaces were called by these names, by shortcuts,
# panel entries and units they set up: the old names stay as links, and the
# program takes the part its name says, with the arguments it always had.
for old in cosmic-ext-applet-taskbar cosmic-ext-applet-start cosmic-ext-applet-central logi-osd; do
    ln -sf "$bin" "$bindir/$old"
done

apps="$prefix/share/applications"
mkdir -p "$apps"
# Taskbar, Start menu and the workspaces' button.
for id in io.github.Aenes0.CosmicTaskbar io.github.Aenes0.CosmicStart io.github.Aenes0.CosmicWorkspacesButton; do
    sed "s|@bindir@|$bindir|" "data/$id.desktop.in" > "$apps/$id.desktop"
done
# The Central's applets: Corner (Central + clock + show desktop), Monitor,
# and the standalone Central / ShowDesktop entries. Custom Tray is omitted so
# COSMIC's native Status Area applet is used directly in Panel Settings.
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
    for unit in central-clipboard central-notifications central-panels logi-osd; do
        systemctl --user enable "$unit.service" >/dev/null 2>&1 || true
        systemctl --user restart "$unit.service" || echo "could not restart $unit now; it starts at the next login"
    done
    for path_unit in fix-brave-pwa-icons wallpapers-monitores; do
        systemctl --user enable --now "$path_unit.path" >/dev/null 2>&1 || true
    done
fi
echo "Installed. Run ./setup.sh to use the taskbar and the Start menu in the panel."
