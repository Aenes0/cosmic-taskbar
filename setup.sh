#!/bin/sh
# Puts the taskbar and the Start menu in the panel and on the keys:
#  - the taskbar where COSMIC's app tray is (the dock keeps its own; the
#    pinned apps are the same in both);
#  - the Start menu in place of COSMIC's launcher and app library: Super
#    (and Super+A, Super+/) opens it, Super+X its tools (Win+X), and its
#    button takes the place of the launcher's and the library's in the panel.
#   ./setup.sh          -> use them
#   ./setup.sh --undo   -> back to COSMIC's own
set -e
bin="${BIN:-$HOME/.local/bin/cosmic-taskbar}"
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic"
panel="$cfg/com.system76.CosmicPanel.Panel/v1"
actions="$cfg/com.system76.CosmicSettings.Shortcuts/v1/system_actions"
custom="$cfg/com.system76.CosmicSettings.Shortcuts/v1/custom"
center="$panel/plugins_center"
start=io.github.Aenes0.CosmicStart
ours=io.github.Aenes0.CosmicTaskbar
theirs=com.system76.CosmicAppList
menu_service="(cosmic-taskbar|cosmic-ext-applet-start) --menu$"

if [ "$1" = "--undo" ]; then
    rm -f "$actions"
    [ -f "$actions.bak" ] && mv "$actions.bak" "$actions"
    if [ -f "$center" ]; then
        sed -i "s|\"$start\",|\"com.system76.CosmicPanelAppButton\",\n    \"com.system76.CosmicPanelLauncherButton\",|" "$center"
    fi
    [ -f "$custom" ] && sed -i "/cosmic-taskbar --tools/d; /cosmic-ext-applet-start --tools/d" "$custom"
    for f in "$panel/plugins_center" "$panel/plugins_wings"; do
        [ -f "$f" ] && sed -i "s|\"$ours\"|\"$theirs\"|" "$f"
    done
    pkill -f "$menu_service" || true
    echo "COSMIC's app tray, launcher and app library are back."
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

# One button for both in the panel, where the launcher's was (or where
# the Start button was, when it was part of the Central).
if [ -f "$center" ] && ! grep -q "\"$start\"" "$center"; then
    sed -i -e "/\"com.system76.CosmicPanelAppButton\",/d" \
        -e "s|\"com.system76.CosmicPanelLauncherButton\",|\"$start\",|" \
        -e "s|\"io.github.Aenes0.CosmicCentral.Start\",|\"$start\",|" "$center"
    grep -q "\"$start\"" "$center" || sed -i "s|^Some(\[|Some([\n    \"$start\",|" "$center"
fi

# The taskbar where COSMIC's app tray is.
for f in "$panel/plugins_center" "$panel/plugins_wings"; do
    [ -f "$f" ] && sed -i "s|\"$theirs\"|\"$ours\"|" "$f"
done
if grep -qs "\"$ours\"" "$panel/plugins_center" "$panel/plugins_wings"; then
    echo "Taskbar and Start menu set: press Super."
else
    echo "No app tray in the panel: add \"Taskbar\" in Settings > Desktop > Panel > Applets."
fi
