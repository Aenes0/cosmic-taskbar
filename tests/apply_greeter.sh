#!/bin/sh
# Tests extras/greeter/apply-greeter.sh without root and without touching the
# system: the script is loaded as a library (APPLY_GREETER_LIB=1), STATE_DIR and
# get_user_home are redefined to a temporary directory, and each case runs in a
# subshell so the parsed globals never leak from one case into the next.
# The script is bash-only (process substitution, arrays), and `sh` may be bash in
# POSIX mode, so re-exec under a real bash first.
[ "${APPLY_TEST_BASH:-}" = 1 ] || APPLY_TEST_BASH=1 exec bash "$0" "$@"
set -e

if [ "$(id -u)" -eq 0 ]; then
    echo "FALHA: nao rode este teste como root" >&2
    exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() { echo "FALHA: $1"; exit 1; }

# Positional parameters of this test must not reach the script's option parser.
set --
APPLY_GREETER_LIB=1
export APPLY_GREETER_LIB
# shellcheck disable=SC1091
. "$root/extras/greeter/apply-greeter.sh"

STATE_DIR="$tmp/state"
GREETER_USER="$(id -un)"
FAKE_HOME="$tmp/home"
mkdir -p "$STATE_DIR" "$FAKE_HOME"
get_user_home() { printf '%s' "$FAKE_HOME"; }

# 1. old style names map to the new ones; the new ones pass through
(
    check_style() {
        printf 'style = "%s"\n' "$1" > "$tmp/style.toml"
        CFG_STYLE="win11"
        parse_greeter_toml "$tmp/style.toml"
        [ "$CFG_STYLE" = "$2" ] || fail "style '$1' deu '$CFG_STYLE', esperado '$2'"
    }
    check_style editorial ios
    check_style sidebar ipados
    check_style macos macos
    check_style ios ios
    check_style ipados ipados
    check_style android android
    check_style win11 win11
    check_style EDITORIAL ios
    check_style bogus win11
)
echo "ok script_maps_old_style_names"

# 2. defaults equal the Rust parser's (greeter/src/style_config.rs)
(
    parse_greeter_toml "$tmp/does-not-exist.toml"
    [ "$CFG_CARD_OPACITY" = "0.0" ] || fail "card_opacity padrao '$CFG_CARD_OPACITY'"
    [ "$CFG_FONT_WEIGHT" = "normal" ] || fail "font_weight padrao '$CFG_FONT_WEIGHT'"
    [ "$CFG_CLOCK_SIZE" = "medium" ] || fail "clock_size padrao '$CFG_CLOCK_SIZE'"
    [ "$CFG_CARD_RADIUS" = "16" ] || fail "card_radius padrao '$CFG_CARD_RADIUS'"
    [ -z "$CFG_SESSION" ] || fail "session padrao deveria ser vazia, veio '$CFG_SESSION'"

    # A file that omits the keys, or carries invalid values, lands on the same defaults.
    cat > "$tmp/invalid.toml" <<'EOT'
font_weight = "ultra"
clock_size = "gigantic"
EOT
    parse_greeter_toml "$tmp/invalid.toml"
    [ "$CFG_FONT_WEIGHT" = "normal" ] || fail "font_weight invalido deu '$CFG_FONT_WEIGHT'"
    [ "$CFG_CLOCK_SIZE" = "medium" ] || fail "clock_size invalido deu '$CFG_CLOCK_SIZE'"
    [ -z "$CFG_SESSION" ] || fail "session ausente deveria ficar vazia, veio '$CFG_SESSION'"
)
echo "ok defaults_match_parser"

# 3. the wallpaper is copied verbatim, whatever blur/dim ask for
(
    head -c 4096 /dev/urandom > "$tmp/original.png"
    cat > "$tmp/wall.toml" <<EOT
wallpaper = "$tmp/original.png"
blur_radius = 20
dim_opacity = 0.5
EOT
    parse_greeter_toml "$tmp/wall.toml"
    DRY_RUN=false
    APPLIED_WALLPAPER=""
    prepare_wallpaper "$FAKE_HOME"
    [ "$APPLIED_WALLPAPER" = "$STATE_DIR/wallpaper.png" ] || fail "APPLIED_WALLPAPER '$APPLIED_WALLPAPER'"
    [ -f "$STATE_DIR/wallpaper.png" ] || fail "wallpaper.png nao foi criado"
    cmp "$tmp/original.png" "$STATE_DIR/wallpaper.png" || fail "wallpaper alterado"
    [ ! -e "$STATE_DIR/wallpaper.jpg" ] || fail "sobrou wallpaper.jpg processado"
)
echo "ok wallpaper_copied_verbatim"

# 4. the current user is no longer pre-selected through the cosmic-greeter state
(
    CFG_DEFAULT_USER="$(id -un)"
    CFG_THEME="dark"
    CFG_CLOCK_24H="true"
    CFG_SHOW_SECONDS="false"
    DRY_RUN=false
    APPLIED_WALLPAPER=""
    write_greeter_cosmic_config
    greeter_cfg="$FAKE_HOME/.config/cosmic/com.system76.CosmicGreeter/v1"
    [ -f "$FAKE_HOME/.config/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark" ] \
        || fail "write_greeter_cosmic_config nao gravou o tema"
    [ ! -e "$greeter_cfg/last_user" ] || fail "last_user foi gravado"
    [ ! -e "$greeter_cfg/users" ] || fail "users foi gravado"
)
echo "ok no_last_user_written"

# 5. the boot cache is created for the greeter user (and only logged on --dry-run)
(
    DRY_RUN=true
    prepare_cache_dir
    [ ! -e "$STATE_DIR/cache" ] || fail "dry-run criou o cache"
    DRY_RUN=false
    prepare_cache_dir
    [ -d "$STATE_DIR/cache" ] || fail "cache nao foi criado"
    [ "$(stat -c '%a' "$STATE_DIR/cache")" = "700" ] || fail "cache deveria ser 0700"
    [ "$(stat -c '%U' "$STATE_DIR/cache")" = "$GREETER_USER" ] || fail "cache de outro dono"
)
echo "ok cache_dir_created"
