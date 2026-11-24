#!/bin/sh
# Puts the taskbar and the Start menu in the panel and on the keys:
#  - the taskbar where COSMIC's app tray is (the dock keeps its own; the
#    pinned apps are the same in both);
#  - the Start menu in place of COSMIC's launcher and app library: Super
#    (and Super+A, Super+/) opens it, Super+X its tools (Win+X), and its
#    button takes the place of the launcher's and the library's in the panel;
#  - the workspaces' button with the taskbar's zoom, where COSMIC's is.
#   ./setup.sh                      -> use them (offers boot greeter & power daemon if interactive)
#   ./setup.sh --with-boot-greeter  -> also enable custom boot login screen (sudo)
#   ./setup.sh --with-power         -> also install economia-energia.service (sudo)
#   ./setup.sh --with-all           -> enables both
#   ./setup.sh --undo               -> back to COSMIC's own
set -e
script_dir="$(dirname "$0")"
with_boot_greeter=0
with_power=0
undo_mode=0
for arg in "$@"; do
    case "$arg" in
        --undo) undo_mode=1 ;;
        --with-boot-greeter) with_boot_greeter=1 ;;
        --with-power) with_power=1 ;;
        --with-all)
            with_boot_greeter=1
            with_power=1
            ;;
    esac
done
bin="${BIN:-$HOME/.local/bin/cosmic-taskbar}"
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic"
panel="$cfg/com.system76.CosmicPanel.Panel/v1"
actions="$cfg/com.system76.CosmicSettings.Shortcuts/v1/system_actions"
custom="$cfg/com.system76.CosmicSettings.Shortcuts/v1/custom"
center="$panel/plugins_center"
start=io.github.Aenes0.CosmicStart
ours=io.github.Aenes0.CosmicTaskbar
theirs=com.system76.CosmicAppList
workspaces=io.github.Aenes0.CosmicWorkspacesButton
their_workspaces=com.system76.CosmicPanelWorkspacesButton
menu_service="(cosmic-taskbar|cosmic-ext-applet-start) --menu$"
. "$script_dir/scripts/corner_wings.sh"

if [ "$undo_mode" -eq 1 ]; then
    corner_undo "$panel/plugins_wings"
    rm -f "$actions"
    [ -f "$actions.bak" ] && mv "$actions.bak" "$actions"
    if [ -f "$center" ]; then
        sed -i "s|\"$start\",|\"com.system76.CosmicPanelAppButton\",\n    \"com.system76.CosmicPanelLauncherButton\",|" "$center"
    fi
    [ -f "$custom" ] && sed -i "/cosmic-taskbar --tools/d; /cosmic-ext-applet-start --tools/d; /io\.github\.Aenes0\.CosmicSwitcher/d" "$custom"
    for f in "$panel/plugins_center" "$panel/plugins_wings"; do
        [ -f "$f" ] && sed -i -e "s|\"$ours\"|\"$theirs\"|" -e "s|\"$workspaces\"|\"$their_workspaces\"|" "$f"
    done
    [ -f "$panel/keyboard_interactivity" ] && echo OnDemand > "$panel/keyboard_interactivity"
    pkill -f "$menu_service" || true
    echo "COSMIC's app tray, launcher, app library and workspaces' button are back."
    exit 0
fi

# The compositor reads these and replaces COSMIC's own, key by key.
mkdir -p "$(dirname "$actions")"
[ -f "$actions" ] && [ ! -f "$actions.bak" ] && ! grep -q "cosmic-taskbar\|cosmic-ext-applet" "$actions" && cp "$actions" "$actions.bak"
cat > "$actions" <<EOT
{
    Launcher: "$bin --menu",
    AppLibrary: "$bin --menu",
}
EOT

# Super+X, the tools' menu, as on Windows (COSMIC has it swap windows).
if ! grep -q "cosmic-taskbar --tools\|cosmic-ext-applet-start --tools" "$custom" 2>/dev/null; then
    line="    (modifiers: [Super], key: \"x\", description: Some(\"Start tools\")): Spawn(\"$bin --tools\"),"
    if [ -s "$custom" ] && grep -q "}" "$custom"; then
        last=$(grep -n "}" "$custom" | tail -1 | cut -d: -f1)
        sed -i "${last}i\\$line" "$custom"
    else
        printf '{\n%s\n}\n' "$line" > "$custom"
    fi
fi

# Super+Shift+S, Windows-style screen snipping (`print-recorte`).
bindir="$(dirname "$bin")"
if ! grep -q "print-recorte" "$custom" 2>/dev/null; then
    line="    (modifiers: [Super, Shift], key: \"s\", description: Some(\"Recorte de tela\")): Spawn(\"$bindir/print-recorte\"),"
    if [ -s "$custom" ] && grep -q "}" "$custom"; then
        last=$(grep -n "}" "$custom" | tail -1 | cut -d: -f1)
        sed -i "${last}i\\$line" "$custom"
    else
        printf '{\n%s\n}\n' "$line" > "$custom"
    fi
fi

# Alt+Tab / Super+Tab: migrate legacy CosmicWorkspaces ShowSwitcherOn bindings
# to our isolated `io.github.Aenes0.CosmicSwitcher` D-Bus service.
if [ -f "$custom" ]; then
    sed -i 's|com\.system76\.CosmicWorkspaces /com/system76/CosmicWorkspaces com\.system76\.CosmicWorkspaces ShowSwitcherOn|io.github.Aenes0.CosmicSwitcher /io/github/Aenes0/CosmicSwitcher io.github.Aenes0.CosmicSwitcher ShowSwitcherOn|g' "$custom"
fi
if ! grep -q "io.github.Aenes0.CosmicSwitcher" "$custom" 2>/dev/null; then
    for entry in \
        '    (modifiers: [Alt], key: "Tab", description: Some("aenes-switcher")): Spawn("busctl --user call io.github.Aenes0.CosmicSwitcher /io/github/Aenes0/CosmicSwitcher io.github.Aenes0.CosmicSwitcher ShowSwitcherOn bs false \"$COSMIC_ACTIVE_OUTPUT\""),' \
        '    (modifiers: [Alt, Shift], key: "Tab", description: Some("aenes-switcher")): Spawn("busctl --user call io.github.Aenes0.CosmicSwitcher /io/github/Aenes0/CosmicSwitcher io.github.Aenes0.CosmicSwitcher ShowSwitcherOn bs true \"$COSMIC_ACTIVE_OUTPUT\""),' \
        '    (modifiers: [Super], key: "Tab", description: Some("aenes-switcher")): Spawn("busctl --user call io.github.Aenes0.CosmicSwitcher /io/github/Aenes0/CosmicSwitcher io.github.Aenes0.CosmicSwitcher ShowSwitcherOn bs false \"$COSMIC_ACTIVE_OUTPUT\""),' \
        '    (modifiers: [Super, Shift], key: "Tab", description: Some("aenes-switcher")): Spawn("busctl --user call io.github.Aenes0.CosmicSwitcher /io/github/Aenes0/CosmicSwitcher io.github.Aenes0.CosmicSwitcher ShowSwitcherOn bs true \"$COSMIC_ACTIVE_OUTPUT\""),'; do
        if [ -s "$custom" ] && grep -q "}" "$custom"; then
            last=$(grep -n "}" "$custom" | tail -1 | cut -d: -f1)
            sed -i "${last}i\\$entry" "$custom"
        else
            printf '{\n%s\n}\n' "$entry" > "$custom"
        fi
    done
fi

# One button for both in the panel, where the launcher's was (or where
# the Start button was, when it was part of the Central).
if [ -f "$center" ] && ! grep -q "\"$start\"" "$center"; then
    sed -i -e "/\"com.system76.CosmicPanelAppButton\",/d" \
        -e "s|\"com.system76.CosmicPanelLauncherButton\",|\"$start\",|" \
        -e "s|\"io.github.Aenes0.CosmicCentral.Start\",|\"$start\",|" "$center"
    grep -q "\"$start\"" "$center" || sed -i "s|^Some(\[|Some([\n    \"$start\",|" "$center"
fi

# The taskbar where COSMIC's app tray is, and the workspaces' button, with
# the taskbar's zoom, where COSMIC's was.
for f in "$panel/plugins_center" "$panel/plugins_wings"; do
    [ -f "$f" ] && sed -i -e "s|\"$theirs\"|\"$ours\"|" -e "s|\"$their_workspaces\"|\"$workspaces\"|" "$f"
done
# The corner's applets become the Canto (which now integrates background apps,
# system bar icons, Central, clock, and show desktop), so remove the external
# COSMIC Status Area applet when Canto is active to avoid duplicate tray icons.
[ -f "$panel/plugins_wings" ] && corner_use "$panel/plugins_wings"
if [ -f "$panel/plugins_wings" ] && grep -q "\"io.github.Aenes0.CosmicCentral.Corner\"" "$panel/plugins_wings"; then
    sed -i '/"com\.system76\.CosmicAppletStatusArea",/d' "$panel/plugins_wings"
fi
# The bar never takes the keys, as on Windows: the compositor gives them to
# every top layer that asks when it maps, so each restart of the panel took
# them from the window in use. Its popups still get them (by their grab),
# and the Start menu and the Central's panels are surfaces of their own.
if [ -d "$panel" ] && [ "$(cat "$panel/keyboard_interactivity" 2>/dev/null)" != None ]; then
    echo None > "$panel/keyboard_interactivity"
fi

if grep -qs "\"$ours\"" "$panel/plugins_center" "$panel/plugins_wings"; then
    echo "Taskbar and Start menu set: press Super."
else
    echo "No app tray in the panel: add \"Taskbar\" in Settings > Desktop > Panel > Applets."
fi

if [ -x "$script_dir/extras/greeter/apply-greeter.sh" ]; then
    if [ "$with_boot_greeter" -eq 1 ]; then
        sudo "$script_dir/extras/greeter/apply-greeter.sh" --user "${USER:-$(id -un)}"
    elif [ -t 0 ] && ! systemctl is-enabled --quiet aenes-greeter.service 2>/dev/null; then
        printf 'Ativar também a tela de login customizada no boot (greetd, requer sudo)? [s/N] '
        read -r ans
        case "$ans" in
            s|S|y|Y|sim|SIM)
                sudo "$script_dir/extras/greeter/apply-greeter.sh" --user "${USER:-$(id -un)}"
                ;;
        esac
    fi
fi

if [ -x "$script_dir/extras/power/install-economia-energia.sh" ]; then
    if [ "$with_power" -eq 1 ]; then
        sudo "$script_dir/extras/power/install-economia-energia.sh"
    elif [ -t 0 ] && ! systemctl is-enabled --quiet economia-energia.service 2>/dev/null; then
        printf 'Instalar também o daemon economia-energia.service (requer sudo)? [s/N] '
        read -r ans
        case "$ans" in
            s|S|y|Y|sim|SIM)
                sudo "$script_dir/extras/power/install-economia-energia.sh"
                ;;
        esac
    fi
fi
