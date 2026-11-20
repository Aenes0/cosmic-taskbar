#!/bin/sh
# Exporta ou aplica o perfil visual completo (COSMIC + fish) no repositório.
# Uso:
#   ./scripts/sync-config.sh export   -> copia ~/.config/cosmic e ~/.config/fish para config/
#   ./scripts/sync-config.sh apply    -> restaura config/ para ~/.config (com backup antes)
set -e
cd "$(dirname "$0")/.."

MODE="${1:-}"
COSMIC_SRC="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic"
FISH_SRC="${XDG_CONFIG_HOME:-$HOME/.config}/fish"
COSMIC_REPO="$PWD/config/cosmic"
FISH_REPO="$PWD/config/fish"

# Somente pastas de aparência, painel, atalhos e applets (sem caches nem segredos).
COSMIC_DIRS="
com.system76.CosmicAppList
com.system76.CosmicAppletAudio
com.system76.CosmicAppletBattery
com.system76.CosmicAppletTime
com.system76.CosmicComp
com.system76.CosmicNotifications
com.system76.CosmicPanel
com.system76.CosmicPanel.Dock
com.system76.CosmicPanel.Panel
com.system76.CosmicPanelButton
com.system76.CosmicSettings.Shortcuts
com.system76.CosmicSettings.WindowRules
com.system76.CosmicTheme.Dark
com.system76.CosmicTheme.Dark.Builder
com.system76.CosmicTheme.Light
com.system76.CosmicTheme.Light.Builder
com.system76.CosmicTheme.Mode
com.system76.CosmicTk
dev.edfloreshz.CosmicTweaks
dev.edfloreshz.CosmicTweaks.ColorScheme
io.github.Aenes0.CosmicCentral
io.github.Aenes0.CosmicCentral.Monitor
io.github.Aenes0.CosmicStart
io.github.Aenes0.CosmicStart.Menu
"

normalize_to_repo() {
    sed "s|$HOME|@HOME@|g" "$1" > "$2"
}

expand_from_repo() {
    sed "s|@HOME@|$HOME|g" "$1" > "$2"
}

case "$MODE" in
    export)
        rm -rf "$COSMIC_REPO"
        mkdir -p "$COSMIC_REPO" "$FISH_REPO/conf.d"
        for d in $COSMIC_DIRS; do
            [ -d "$COSMIC_SRC/$d" ] || continue
            find "$COSMIC_SRC/$d" -type f ! -name "*.bak" ! -name ".*" | while IFS= read -r f; do
                rel="${f#"$COSMIC_SRC/"}"
                mkdir -p "$COSMIC_REPO/$(dirname "$rel")"
                normalize_to_repo "$f" "$COSMIC_REPO/$rel"
            done
        done
        if [ -f "$FISH_SRC/config.fish" ]; then
            normalize_to_repo "$FISH_SRC/config.fish" "$FISH_REPO/config.fish"
        fi
        if [ -d "$FISH_SRC/conf.d" ]; then
            for f in "$FISH_SRC/conf.d"/*.fish; do
                [ -f "$f" ] || continue
                normalize_to_repo "$f" "$FISH_REPO/conf.d/$(basename "$f")"
            done
        fi
        echo "Exportado para config/cosmic e config/fish."
        ;;
    apply)
        stamp="$(date +%Y%m%d-%H%M%S)"
        if [ -d "$COSMIC_SRC" ]; then
            cp -a "$COSMIC_SRC" "${COSMIC_SRC}.backup-${stamp}"
            echo "Backup salvo em ${COSMIC_SRC}.backup-${stamp}"
        fi
        mkdir -p "$COSMIC_SRC" "$FISH_SRC/conf.d"
        if [ -d "$COSMIC_REPO" ]; then
            find "$COSMIC_REPO" -type f | while IFS= read -r f; do
                rel="${f#"$COSMIC_REPO/"}"
                mkdir -p "$COSMIC_SRC/$(dirname "$rel")"
                expand_from_repo "$f" "$COSMIC_SRC/$rel"
            done
        fi
        if [ -f "$FISH_REPO/config.fish" ]; then
            expand_from_repo "$FISH_REPO/config.fish" "$FISH_SRC/config.fish"
        fi
        if [ -d "$FISH_REPO/conf.d" ]; then
            for f in "$FISH_REPO/conf.d"/*.fish; do
                [ -f "$f" ] || continue
                expand_from_repo "$f" "$FISH_SRC/conf.d/$(basename "$f")"
            done
        fi
        echo "Perfil aplicado em ~/.config/cosmic e ~/.config/fish."
        ;;
    *)
        echo "Uso: $0 {export|apply}" >&2
        exit 2
        ;;
esac
