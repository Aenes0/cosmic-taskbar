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
# Taskbar and Start menu.
for id in io.github.Aenes0.CosmicTaskbar io.github.Aenes0.CosmicStart; do
    sed "s|@bindir@|$bindir|" "data/$id.desktop.in" > "$apps/$id.desktop"
done
# The Central's applets: quick settings, show desktop, system monitor, tray
# (the tray keeps the id it had as its own program, and its place on the panel).
central=io.github.Aenes0.CosmicCentral
for id in "$central" "$central.ShowDesktop" "$central.Monitor" io.github.Aenes0.CosmicTray; do
    sed "s|@bindir@|$bindir|" "data/central/$id.desktop.in" > "$apps/$id.desktop"
done
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

# The Logitech devices' service, where it always had its unit.
logi_unit="$HOME/.config/systemd/user/logi-osd.service"
mkdir -p "$(dirname "$logi_unit")"
sed "s|@bindir@|$bindir|" data/logi/logi-osd.service > "$logi_unit"
install -Dm0644 data/logi/config.exemplo "$prefix/share/logi-osd/config.exemplo"
if [ ! -e /etc/udev/rules.d/70-logi-osd.rules ] && [ ! -e /usr/lib/udev/rules.d/42-logitech-unify-permissions.rules ]; then
    echo "The Logitech service needs /dev/hidraw* access; as root, once:"
    echo "  sudo install -Dm0644 data/logi/70-logi-osd.rules /etc/udev/rules.d/70-logi-osd.rules && sudo udevadm control --reload && sudo udevadm trigger --subsystem-match=hidraw"
fi

# Only a plain ~/.local install starts the services here; system prefixes
# leave that to the packager. Restarting the panels' host closes an open
# panel, nothing else: the bar is not touched.
if [ "$prefix" = "$HOME/.local" ]; then
    systemctl --user daemon-reload
    for unit in central-clipboard central-notifications central-panels logi-osd; do
        systemctl --user enable "$unit.service" >/dev/null 2>&1 || true
        systemctl --user restart "$unit.service" || echo "could not restart $unit now; it starts at the next login"
    done
fi
echo "Installed. Run ./setup.sh to use the taskbar and the Start menu in the panel."
