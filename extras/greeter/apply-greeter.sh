#!/usr/bin/env bash
# Aplica a configuração isolada da tela de login (~/.config/cosmic-taskbar/greeter.toml)
# sem tocar nos arquivos gerenciados pelo pacote oficial `cosmic-greeter` (pacman).
#
# Arquivos gerenciados por este script (100% isolados):
#   - /usr/local/bin/aenes-greeter            (binário customizado, se isolated_binary = true)
#   - /usr/local/bin/aenes-greeter-start      (wrapper que sobe cosmic-comp com o binário escolhido)
#   - /usr/local/bin/apply-greeter            (atalho estável para pkexec / Painel de Controle)
#   - /etc/greetd/aenes-greeter.toml          (config greetd dedicada)
#   - /etc/systemd/system/aenes-greeter.service (unit display-manager dedicada)
#   - /var/lib/aenes-greeter/                 (wallpaper processado, env, snapshot e backups)
#   - /var/lib/cosmic-greeter/.config/cosmic/ (preferências cosmic-config do greeter)
#
# Uso:
#   sudo ./apply-greeter.sh [--config PATH] [--user USER] [--source DIR] [--rebuild] [--dry-run]
#   sudo ./apply-greeter.sh --restore
#   ./apply-greeter.sh --status

set -euo pipefail

SCRIPT_PATH="$(readlink -f "$0" 2>/dev/null || echo "$0")"
SCRIPT_DIR="$(dirname "$SCRIPT_PATH")"

STATE_DIR="/var/lib/aenes-greeter"
BACKUP_ROOT="$STATE_DIR/backups"
GREETD_CFG="/etc/greetd/aenes-greeter.toml"
SYSTEMD_UNIT="/etc/systemd/system/aenes-greeter.service"
START_WRAPPER="/usr/local/bin/aenes-greeter-start"
ISOLATED_BIN="/usr/local/bin/aenes-greeter"
ISOLATED_DAEMON="/usr/local/bin/aenes-greeter-daemon"
APPLY_BIN="/usr/local/bin/apply-greeter"
POLKIT_DST="/usr/share/polkit-1/actions/io.github.Aenes0.CosmicGreeterApply.policy"

# Defaults do schema ~/.config/cosmic-taskbar/greeter.toml
CFG_STYLE="win11"
CFG_WALLPAPER=""
CFG_WALLPAPER_MODE="fixed"
CFG_WALLPAPER_DIR=""
CFG_WALLPAPER_SHUFFLE="true"
CFG_WALLPAPER_INTERVAL_MINS="30"
CFG_CLOCK_24H="true"
CFG_SHOW_SECONDS="false"
CFG_SHOW_DATE="true"
CFG_SHOW_AVATAR="true"
CFG_SHOW_USERNAME="true"
CFG_FONT_FAMILY="system"
CFG_FONT_WEIGHT="light"
CFG_CLOCK_SIZE="large"
CFG_BLUR_RADIUS="0"
CFG_DIM_OPACITY="0.0"
CFG_CARD_OPACITY="0.58"
CFG_CARD_RADIUS="18"
CFG_ACCENT_COLOR=""
CFG_THEME="dark"
CFG_DEFAULT_USER=""
CFG_SESSION="COSMIC"
CFG_ISOLATED_BINARY="true"

# Flags CLI
MODE="apply"
CONFIG_FILE=""
TARGET_USER=""
SOURCE_DIR="${AENES_GREETER_SRC:-}"
REBUILD="false"
DRY_RUN="false"

usage() {
    cat <<'EOF'
Uso: apply-greeter.sh [opções]

Opções:
  --config <caminho>   Caminho para greeter.toml (padrão: ~/.config/cosmic-taskbar/greeter.toml)
  --user <usuario>     Usuário dono do greeter.toml (detectado via SUDO_USER/PKEXEC_UID)
  --source <pasta>     Pasta do aenes-greeter (padrão: greeter/ dentro do cosmic-taskbar)
  --rebuild            Força recompilação de /usr/local/bin/aenes-greeter
  --install-only       Instala apenas /usr/local/bin/apply-greeter e política Polkit
  --restore            Desativa aenes-greeter.service e reativa cosmic-greeter.service oficial
  --status             Mostra o estado atual do greeter isolado (não requer root)
  --dry-run            Mostra as ações sem gravar arquivos no sistema
  -h, --help           Exibe esta ajuda
EOF
}

log() {
    printf '[aenes-greeter] %s\n' "$*"
}

warn() {
    printf '[aenes-greeter] AVISO: %s\n' "$*" >&2
}

die() {
    printf '[aenes-greeter] ERRO: %s\n' "$*" >&2
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --config)
            [ "$#" -ge 2 ] || die "--config requer um caminho"
            CONFIG_FILE="$2"
            shift 2
            ;;
        --user)
            [ "$#" -ge 2 ] || die "--user requer um nome de usuário"
            TARGET_USER="$2"
            shift 2
            ;;
        --source)
            [ "$#" -ge 2 ] || die "--source requer um diretório"
            SOURCE_DIR="$2"
            shift 2
            ;;
        --rebuild)
            REBUILD="true"
            shift
            ;;
        --install-only)
            MODE="install-only"
            shift
            ;;
        --restore)
            MODE="restore"
            shift
            ;;
        --status)
            MODE="status"
            shift
            ;;
        --dry-run)
            DRY_RUN="true"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "Opção desconhecida: $1 (use --help)"
            ;;
    esac
done

resolve_target_user() {
    if [ -n "$TARGET_USER" ]; then
        printf '%s' "$TARGET_USER"
        return
    fi
    if [ -n "${SUDO_USER:-}" ] && [ "${SUDO_USER}" != "root" ]; then
        printf '%s' "$SUDO_USER"
        return
    fi
    if [ -n "${PKEXEC_UID:-}" ] && [ "${PKEXEC_UID}" != "0" ]; then
        getent passwd "$PKEXEC_UID" | cut -d: -f1
        return
    fi
    printf '%s' "${USER:-root}"
}

get_user_home() {
    local u="$1"
    local h
    h="$(getent passwd "$u" 2>/dev/null | cut -d: -f6 || true)"
    if [ -n "$h" ]; then
        printf '%s' "$h"
    else
        printf '%s' "${HOME:-/root}"
    fi
}

detect_greeter_user() {
    if id -u cosmic-greeter >/dev/null 2>&1; then
        printf 'cosmic-greeter'
    elif id -u _greetd >/dev/null 2>&1; then
        printf '_greetd'
    elif id -u greeter >/dev/null 2>&1; then
        printf 'greeter'
    else
        printf 'cosmic-greeter'
    fi
}

trim() {
    local s="$1"
    s="${s#"${s%%[![:space:]]*}"}"
    s="${s%"${s##*[![:space:]]}"}"
    printf '%s' "$s"
}

strip_inline_comment() {
    local s="$1"
    local quote=""
    local escaped="false"
    local out=""
    local i ch
    for ((i = 0; i < ${#s}; i++)); do
        ch="${s:i:1}"
        if [ "$escaped" = "true" ]; then
            out+="$ch"
            escaped="false"
            continue
        fi
        if [ "$quote" = '"' ]; then
            if [ "$ch" = "\\" ]; then
                out+="$ch"
                escaped="true"
            elif [ "$ch" = '"' ]; then
                out+="$ch"
                quote=""
            else
                out+="$ch"
            fi
        elif [ "$quote" = "'" ]; then
            out+="$ch"
            if [ "$ch" = "'" ]; then
                quote=""
            fi
        else
            if [ "$ch" = '"' ] || [ "$ch" = "'" ]; then
                out+="$ch"
                quote="$ch"
            elif [ "$ch" = "#" ]; then
                break
            else
                out+="$ch"
            fi
        fi
    done
    trim "$out"
}

unquote_toml_string() {
    local v="$1"
    if [[ "$v" == \"*\" ]]; then
        v="${v#\"}"
        v="${v%\"}"
        v="${v//\\\"/\"}"
        v="${v//\\\\/\\}"
    elif [[ "$v" == \'*\' ]]; then
        v="${v#\'}"
        v="${v%\'}"
    fi
    printf '%s' "$v"
}

parse_bool() {
    local raw="$1"
    local fallback="$2"
    case "${raw,,}" in
        true|1|yes) printf 'true' ;;
        false|0|no) printf 'false' ;;
        *) printf '%s' "$fallback" ;;
    esac
}

parse_greeter_toml() {
    local file="$1"
    [ -f "$file" ] || return 0

    local line clean key val
    while IFS= read -r line || [ -n "$line" ]; do
        clean="$(strip_inline_comment "$line")"
        [ -n "$clean" ] || continue
        [[ "$clean" == \[*\] ]] && continue
        [[ "$clean" == *=* ]] || continue

        key="$(trim "${clean%%=*}")"
        val="$(trim "${clean#*=}")"

        case "$key" in
            style)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    win11|android|macos|sidebar|editorial) CFG_STYLE="${val,,}" ;;
                    *) CFG_STYLE="win11" ;;
                esac
                ;;
            wallpaper)
                CFG_WALLPAPER="$(unquote_toml_string "$val")"
                ;;
            wallpaper_mode)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    fixed|rotate_on_lock|rotate_interval) CFG_WALLPAPER_MODE="${val,,}" ;;
                    *) CFG_WALLPAPER_MODE="fixed" ;;
                esac
                ;;
            wallpaper_dir)
                CFG_WALLPAPER_DIR="$(unquote_toml_string "$val")"
                ;;
            wallpaper_shuffle)
                CFG_WALLPAPER_SHUFFLE="$(parse_bool "$val" "$CFG_WALLPAPER_SHUFFLE")"
                ;;
            wallpaper_interval_mins)
                if [[ "$val" =~ ^[0-9]+$ ]] && [ "$val" -ge 1 ]; then
                    CFG_WALLPAPER_INTERVAL_MINS="$val"
                fi
                ;;
            font_family)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    system|sans|mono|serif) CFG_FONT_FAMILY="${val,,}" ;;
                    *) CFG_FONT_FAMILY="system" ;;
                esac
                ;;
            font_weight)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    light|normal|semibold|bold) CFG_FONT_WEIGHT="${val,,}" ;;
                    *) CFG_FONT_WEIGHT="light" ;;
                esac
                ;;
            clock_size)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    small|medium|large|huge) CFG_CLOCK_SIZE="${val,,}" ;;
                    *) CFG_CLOCK_SIZE="large" ;;
                esac
                ;;
            clock_24h)
                CFG_CLOCK_24H="$(parse_bool "$val" "$CFG_CLOCK_24H")"
                ;;
            show_seconds)
                CFG_SHOW_SECONDS="$(parse_bool "$val" "$CFG_SHOW_SECONDS")"
                ;;
            show_date)
                CFG_SHOW_DATE="$(parse_bool "$val" "$CFG_SHOW_DATE")"
                ;;
            show_avatar)
                CFG_SHOW_AVATAR="$(parse_bool "$val" "$CFG_SHOW_AVATAR")"
                ;;
            show_username)
                CFG_SHOW_USERNAME="$(parse_bool "$val" "$CFG_SHOW_USERNAME")"
                ;;
            blur_radius)
                if [[ "$val" =~ ^[0-9]+$ ]]; then
                    CFG_BLUR_RADIUS="$val"
                elif [[ "$val" =~ ^[0-9]+\.[0-9]+$ ]]; then
                    CFG_BLUR_RADIUS="${val%%.*}"
                fi
                ;;
            dim_opacity)
                if [[ "$val" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
                    CFG_DIM_OPACITY="$val"
                fi
                ;;
            card_opacity)
                if [[ "$val" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
                    CFG_CARD_OPACITY="$val"
                fi
                ;;
            card_radius)
                if [[ "$val" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
                    CFG_CARD_RADIUS="${val%%.*}"
                fi
                ;;
            accent_color)
                val="$(unquote_toml_string "$val")"
                if [ -z "$val" ] || [[ "$val" =~ ^#[0-9A-Fa-f]{6}$ ]]; then
                    CFG_ACCENT_COLOR="$val"
                fi
                ;;
            theme)
                val="$(unquote_toml_string "$val")"
                case "${val,,}" in
                    light) CFG_THEME="light" ;;
                    *) CFG_THEME="dark" ;;
                esac
                ;;
            default_user)
                CFG_DEFAULT_USER="$(unquote_toml_string "$val")"
                ;;
            session)
                CFG_SESSION="$(unquote_toml_string "$val")"
                ;;
            isolated_binary)
                CFG_ISOLATED_BINARY="$(parse_bool "$val" "$CFG_ISOLATED_BINARY")"
                ;;
        esac
    done < "$file"

    # Clamp blur_radius em 0..60
    if [ "$CFG_BLUR_RADIUS" -lt 0 ] 2>/dev/null; then
        CFG_BLUR_RADIUS="0"
    elif [ "$CFG_BLUR_RADIUS" -gt 60 ] 2>/dev/null; then
        CFG_BLUR_RADIUS="60"
    fi

    # Clamp dim_opacity em 0.0..0.85 (LC_ALL=C garante ponto decimal independente do locale)
    CFG_DIM_OPACITY="$(LC_ALL=C awk -v v="$CFG_DIM_OPACITY" 'BEGIN {
        if (v < 0.0) v = 0.0;
        if (v > 0.85) v = 0.85;
        printf "%.2f", v;
    }')"

    # Validação de default_user para evitar caracteres inválidos
    if [ -n "$CFG_DEFAULT_USER" ] && ! [[ "$CFG_DEFAULT_USER" =~ ^[A-Za-z0-9._-]+$ ]]; then
        warn "default_user inválido ('$CFG_DEFAULT_USER'), ignorando."
        CFG_DEFAULT_USER=""
    fi

    [ -n "$CFG_SESSION" ] || CFG_SESSION="COSMIC"
}

ron_escape() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    printf '%s' "$s"
}

toml_escape() {
    ron_escape "$1"
}

show_status() {
    local target_user target_home cfg dm_target
    target_user="$(resolve_target_user)"
    target_home="$(get_user_home "$target_user")"
    cfg="${CONFIG_FILE:-$target_home/.config/cosmic-taskbar/greeter.toml}"
    dm_target="$(readlink -f /etc/systemd/system/display-manager.service 2>/dev/null || echo "desconhecido")"

    echo "=== Aenes Greeter Isolado ==="
    echo "display-manager.service -> $dm_target"
    echo "aenes-greeter.service   : $(systemctl is-enabled aenes-greeter.service 2>/dev/null || echo 'não instalado')"
    echo "cosmic-greeter.service  : $(systemctl is-enabled cosmic-greeter.service 2>/dev/null || echo 'desativado')"
    echo "binário isolado         : $([ -x "$ISOLATED_BIN" ] && echo "$ISOLATED_BIN (presente)" || echo "$ISOLATED_BIN (ausente)")"
    echo "wrapper start           : $([ -x "$START_WRAPPER" ] && echo "$START_WRAPPER (presente)" || echo "$START_WRAPPER (ausente)")"
    echo "config greetd isolada   : $([ -f "$GREETD_CFG" ] && echo "$GREETD_CFG (presente)" || echo "$GREETD_CFG (ausente)")"
    echo "config usuário          : $cfg $([ -f "$cfg" ] && echo '(presente)' || echo '(ausente)')"
    if [ -f "$STATE_DIR/greeter.toml" ]; then
        echo "--- Última configuração aplicada ($STATE_DIR/greeter.toml) ---"
        cat "$STATE_DIR/greeter.toml"
    fi
}

require_root() {
    if [ "$DRY_RUN" = "true" ]; then
        return 0
    fi
    if [ "${EUID:-$(id -u)}" -ne 0 ]; then
        die "Execute como root (ex.: pkexec /usr/local/bin/apply-greeter ou sudo $0)."
    fi
}

install_helper_and_polkit() {
    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] Instalaria $APPLY_BIN e $POLKIT_DST"
        return 0
    fi
    if [ "$SCRIPT_PATH" != "$APPLY_BIN" ] && [ -f "$SCRIPT_PATH" ]; then
        install -Dm0755 "$SCRIPT_PATH" "$APPLY_BIN"
        log "Instalado $APPLY_BIN"
    fi
    if [ -f "$SCRIPT_DIR/io.github.Aenes0.CosmicGreeterApply.policy" ]; then
        install -Dm0644 "$SCRIPT_DIR/io.github.Aenes0.CosmicGreeterApply.policy" "$POLKIT_DST"
        log "Instalada política Polkit em $POLKIT_DST"
    fi
}

restore_official_greeter() {
    require_root
    log "Restaurando display-manager para cosmic-greeter.service oficial..."
    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] systemctl disable aenes-greeter.service"
        log "[dry-run] systemctl enable cosmic-greeter.service"
        return 0
    fi
    systemctl disable aenes-greeter.service 2>/dev/null || true
    if systemctl list-unit-files cosmic-greeter.service >/dev/null 2>&1; then
        systemctl enable --force cosmic-greeter.service
    fi
    systemctl daemon-reload
    log "Greeter oficial restaurado (terá efeito no próximo boot/logout). Arquivos em $STATE_DIR mantidos."
}

backup_existing_state() {
    local stamp backup_dir greeter_home
    stamp="$(date +%Y%m%d-%H%M%S)"
    backup_dir="$BACKUP_ROOT/$stamp"
    greeter_home="$(get_user_home "$GREETER_USER")"

    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] Criaria backup automático em $backup_dir"
        return 0
    fi

    install -dm0755 "$STATE_DIR" "$backup_dir"
    if [ ! -f "$STATE_DIR/display-manager.orig" ] && [ -L /etc/systemd/system/display-manager.service ]; then
        readlink -f /etc/systemd/system/display-manager.service > "$STATE_DIR/display-manager.orig"
    fi

    for f in "$GREETD_CFG" "$SYSTEMD_UNIT" "$START_WRAPPER" "$STATE_DIR/greeter.toml" "$STATE_DIR/env"; do
        [ -f "$f" ] && cp -a "$f" "$backup_dir/"
    done
    if [ -d "$greeter_home/.config/cosmic/com.system76.CosmicGreeter" ]; then
        cp -a "$greeter_home/.config/cosmic/com.system76.CosmicGreeter" "$backup_dir/CosmicGreeter-config"
    fi

    # Mantém no máximo os 10 backups mais recentes
    local old
    while IFS= read -r old; do
        [ -n "$old" ] && rm -rf "$old"
    done < <(ls -1dt "$BACKUP_ROOT"/* 2>/dev/null | tail -n +11 || true)

    log "Backup salvo em $backup_dir"
}

needs_compile() {
    local src_dir="$1"
    local built_bin="$src_dir/target/release/cosmic-greeter"
    [ "$REBUILD" = "true" ] && return 0
    [ ! -x "$built_bin" ] && return 0
    [ -n "$(find "$src_dir/src" "$src_dir/Cargo.toml" "$src_dir/cosmic-greeter-config" "$src_dir/daemon" -type f -newer "$built_bin" 2>/dev/null | head -n 1)" ]
}

needs_install_bin() {
    local src_dir="$1"
    local built_bin="$src_dir/target/release/cosmic-greeter"
    [ "$REBUILD" = "true" ] && return 0
    [ ! -x "$ISOLATED_BIN" ] && return 0
    [ -x "$built_bin" ] && [ "$built_bin" -nt "$ISOLATED_BIN" ] && return 0
    [ -n "$(find "$src_dir/src" "$src_dir/Cargo.toml" "$src_dir/cosmic-greeter-config" "$src_dir/daemon" -type f -newer "$ISOLATED_BIN" 2>/dev/null | head -n 1)" ]
}

ensure_isolated_binary() {
    local src_dir="$1"
    if [ "$CFG_ISOLATED_BINARY" != "true" ]; then
        ACTIVE_GREETER_BIN="/usr/bin/cosmic-greeter"
        log "isolated_binary=false: usando binário oficial $ACTIVE_GREETER_BIN (com unit e configs isoladas)."
        return 0
    fi

    if [ -d "$src_dir" ] && [ -f "$src_dir/Cargo.toml" ]; then
        if needs_install_bin "$src_dir"; then
            local owner owner_home cargo_bin
            owner="$(stat -c '%U' "$src_dir")"
            if [ "$owner" = "root" ] && [ -n "${TARGET_USER:-}" ] && [ "$TARGET_USER" != "root" ]; then
                owner="$TARGET_USER"
            fi
            if [ "$owner" = "root" ]; then
                die "Recusado compilar cargo como root por segurança (ajuste o dono de $src_dir)."
            fi
            owner_home="$(get_user_home "$owner")"
            cargo_bin="$owner_home/.cargo/bin"
            if needs_compile "$src_dir"; then
                log "Compilando greeter customizado em $src_dir (como usuário $owner)..."
                if [ "$DRY_RUN" = "true" ]; then
                    log "[dry-run] Compilaria cargo build --release e instalaria em $ISOLATED_BIN"
                else
                    if command -v runuser >/dev/null 2>&1; then
                        runuser -u "$owner" -- env PATH="$cargo_bin:/usr/local/bin:/usr/bin:/bin" HOME="$owner_home" \
                            cargo build --release --manifest-path "$src_dir/Cargo.toml" --bin cosmic-greeter
                    else
                        su -s /bin/sh "$owner" -c "PATH='$cargo_bin:/usr/local/bin:/usr/bin:/bin' HOME='$owner_home' cargo build --release --manifest-path '$src_dir/Cargo.toml' --bin cosmic-greeter"
                    fi
                fi
            fi
            if [ "$DRY_RUN" = "true" ]; then
                log "[dry-run] Instalaria $src_dir/target/release/cosmic-greeter em $ISOLATED_BIN"
            else
                install -Dm0755 "$src_dir/target/release/cosmic-greeter" "$ISOLATED_BIN"
                if [ -x "$src_dir/target/release/cosmic-greeter-daemon" ]; then
                    install -Dm0755 "$src_dir/target/release/cosmic-greeter-daemon" "$ISOLATED_DAEMON"
                fi
                log "Binário isolado instalado em $ISOLATED_BIN"
            fi
        else
            log "Binário isolado $ISOLATED_BIN já está atualizado."
        fi
        ACTIVE_GREETER_BIN="$ISOLATED_BIN"
    elif [ -x "$ISOLATED_BIN" ]; then
        log "Usando binário isolado já instalado: $ISOLATED_BIN"
        ACTIVE_GREETER_BIN="$ISOLATED_BIN"
    else
        warn "Repositório $src_dir e $ISOLATED_BIN não encontrados; fallback para /usr/bin/cosmic-greeter."
        ACTIVE_GREETER_BIN="/usr/bin/cosmic-greeter"
    fi
}

prepare_wallpaper() {
    local target_home="$1"
    APPLIED_WALLPAPER=""

    if [ -z "$CFG_WALLPAPER" ]; then
        if [ "$DRY_RUN" = "true" ]; then
            log "[dry-run] Sem wallpaper fixo: limparia $STATE_DIR/wallpaper.*"
        else
            rm -f "$STATE_DIR"/wallpaper.* "$STATE_DIR"/wallpaper-source
        fi
        log "Wallpaper vazio: tela de login usará o papel de parede do usuário."
        return 0
    fi

    local src="$CFG_WALLPAPER"
    if [[ "$src" == "~/"* ]]; then
        src="$target_home/${src#\~/}"
    fi
    if [ ! -f "$src" ]; then
        warn "Wallpaper não encontrado: $src (mantendo padrão do usuário)."
        return 0
    fi

    local ext="${src##*.}"
    ext="${ext,,}"
    case "$ext" in
        jpg|jpeg|png|webp|bmp|jxl|avif) ;;
        *) ext="jpg" ;;
    esac

    local need_fx="false"
    if [ "$CFG_BLUR_RADIUS" -gt 0 ] || LC_ALL=C awk -v d="$CFG_DIM_OPACITY" 'BEGIN { exit !(d > 0.001) }'; then
        need_fx="true"
    fi

    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] Copiaria/processaria $src -> $STATE_DIR/wallpaper.jpg (blur=$CFG_BLUR_RADIUS, dim=$CFG_DIM_OPACITY)"
        APPLIED_WALLPAPER="$STATE_DIR/wallpaper.jpg"
        return 0
    fi

    rm -f "$STATE_DIR"/wallpaper.* "$STATE_DIR"/wallpaper-source
    install -Dm0644 "$src" "$STATE_DIR/wallpaper-source"

    if [ "$need_fx" = "true" ] && command -v python3 >/dev/null 2>&1 && python3 -c "import PIL" >/dev/null 2>&1; then
        local dst="$STATE_DIR/wallpaper.jpg"
        python3 - "$src" "$dst" "$CFG_BLUR_RADIUS" "$CFG_DIM_OPACITY" <<'PYEOF'
import sys
from PIL import Image, ImageFilter

src, dst, blur_s, dim_s = sys.argv[1:5]
blur = max(0.0, float(blur_s))
dim = max(0.0, min(0.85, float(dim_s)))

with Image.open(src) as img:
    img = img.convert("RGB")
    if blur > 0.0:
        img = img.filter(ImageFilter.GaussianBlur(radius=blur))
    if dim > 0.0:
        black = Image.new("RGB", img.size, (0, 0, 0))
        img = Image.blend(img, black, dim)
    img.save(dst, "JPEG", quality=94, optimize=True)
PYEOF
        chmod 0644 "$dst"
        APPLIED_WALLPAPER="$dst"
        log "Wallpaper processado (blur=${CFG_BLUR_RADIUS}px, dim=${CFG_DIM_OPACITY}) em $dst"
    elif [ "$need_fx" = "true" ] && (command -v magick >/dev/null 2>&1 || command -v convert >/dev/null 2>&1); then
        local dst="$STATE_DIR/wallpaper.jpg"
        local im_cmd="convert"
        command -v magick >/dev/null 2>&1 && im_cmd="magick"
        local dim_pct
        dim_pct="$(LC_ALL=C awk -v d="$CFG_DIM_OPACITY" 'BEGIN { printf "%d", d * 100 }')"
        local args=("$src")
        [ "$CFG_BLUR_RADIUS" -gt 0 ] && args+=(-blur "0x${CFG_BLUR_RADIUS}")
        [ "$dim_pct" -gt 0 ] && args+=(-fill black -colorize "${dim_pct}%")
        args+=(-quality 94 "$dst")
        "$im_cmd" "${args[@]}"
        chmod 0644 "$dst"
        APPLIED_WALLPAPER="$dst"
        log "Wallpaper processado via $im_cmd (blur=${CFG_BLUR_RADIUS}px, dim=${CFG_DIM_OPACITY}) em $dst"
    else
        local dst="$STATE_DIR/wallpaper.$ext"
        install -Dm0644 "$src" "$dst"
        APPLIED_WALLPAPER="$dst"
        if [ "$need_fx" = "true" ]; then
            warn "Pillow/ImageMagick não encontrados; wallpaper copiado sem blur/dim para $dst"
        else
            log "Wallpaper copiado para $dst"
        fi
    fi
}

write_greeter_cosmic_config() {
    local greeter_home cfg_root state_root is_dark uid=""
    greeter_home="$(get_user_home "$GREETER_USER")"
    cfg_root="$greeter_home/.config/cosmic"
    state_root="$greeter_home/.local/state/cosmic"

    [ "$CFG_THEME" = "light" ] && is_dark="false" || is_dark="true"

    if [ -n "$CFG_DEFAULT_USER" ]; then
        uid="$(id -u "$CFG_DEFAULT_USER" 2>/dev/null || true)"
        if [ -z "$uid" ] || [ "$uid" = "0" ]; then
            warn "default_user '$CFG_DEFAULT_USER' não existe (ou UID=0); ignorando pré-seleção."
            uid=""
        fi
    fi

    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] Gravaria cosmic-config do greeter em $cfg_root (is_dark=$is_dark, 24h=$CFG_CLOCK_24H, seconds=$CFG_SHOW_SECONDS, uid=${uid:-none}, session=$CFG_SESSION)"
        return 0
    fi

    install -dm0750 -o "$GREETER_USER" -g "$GREETER_USER" "$greeter_home"
    install -dm0755 -o "$GREETER_USER" -g "$GREETER_USER" \
        "$cfg_root/com.system76.CosmicGreeter/v1" \
        "$cfg_root/com.system76.CosmicTheme.Mode/v1" \
        "$cfg_root/com.system76.CosmicAppletTime/v1" \
        "$cfg_root/com.system76.CosmicBackground/v1" \
        "$state_root/com.system76.CosmicBackground/v1"

    printf '%s\n' "$is_dark" > "$cfg_root/com.system76.CosmicTheme.Mode/v1/is_dark"
    printf '%s\n' "$CFG_CLOCK_24H" > "$cfg_root/com.system76.CosmicAppletTime/v1/military_time"
    printf '%s\n' "$CFG_SHOW_SECONDS" > "$cfg_root/com.system76.CosmicAppletTime/v1/show_seconds"

    if [ -n "$uid" ]; then
        local esc_session
        esc_session="$(ron_escape "$CFG_SESSION")"
        printf 'Some(%s)\n' "$uid" > "$cfg_root/com.system76.CosmicGreeter/v1/last_user"
        cat > "$cfg_root/com.system76.CosmicGreeter/v1/users" <<EOF
{
    $uid: (
        uid: $uid,
        last_session: Some("$esc_session"),
    ),
}
EOF
    fi

    if [ -n "$APPLIED_WALLPAPER" ]; then
        local esc_wp
        esc_wp="$(ron_escape "$APPLIED_WALLPAPER")"
        printf 'true\n' > "$cfg_root/com.system76.CosmicBackground/v1/same-on-all"
        cat > "$cfg_root/com.system76.CosmicBackground/v1/all" <<EOF
(
    output: "all",
    source: Path("$esc_wp"),
    filter_by_theme: false,
    rotation_frequency: 3600,
    filter_method: Lanczos,
    scaling_mode: Zoom,
    sampling_method: Alphanumeric,
)
EOF
        cat > "$state_root/com.system76.CosmicBackground/v1/wallpapers" <<EOF
[
    ("", Path("$esc_wp")),
]
EOF
    else
        if [ -f "$state_root/com.system76.CosmicBackground/v1/wallpapers" ] \
            && grep -q "$STATE_DIR/" "$state_root/com.system76.CosmicBackground/v1/wallpapers" 2>/dev/null; then
            rm -f "$state_root/com.system76.CosmicBackground/v1/wallpapers"
        fi
    fi

    chown -R "$GREETER_USER:$GREETER_USER" "$greeter_home/.config" "$greeter_home/.local"
    log "Preferências cosmic-config aplicadas em $cfg_root"
}

write_isolated_runtime_files() {
    local esc_wp esc_wp_dir esc_user esc_session esc_accent
    local mirrored_wp_dir=""
    if [ "$CFG_WALLPAPER_MODE" != "fixed" ] && [ -n "$CFG_WALLPAPER_DIR" ]; then
        local src_wp_dir="$CFG_WALLPAPER_DIR"
        if [[ "$src_wp_dir" == "~/"* ]]; then
            src_wp_dir="$TARGET_HOME/${src_wp_dir#\~/}"
        fi
        if [ -d "$src_wp_dir" ] && [ "$DRY_RUN" != "true" ]; then
            mirrored_wp_dir="$STATE_DIR/wallpapers"
            rm -rf "$mirrored_wp_dir"
            install -dm0755 "$mirrored_wp_dir"
            local img_f img_count=0
            while IFS= read -r -d '' img_f; do
                install -m0644 "$img_f" "$mirrored_wp_dir/$(basename "$img_f")"
                img_count=$((img_count + 1))
                [ "$img_count" -ge 100 ] && break
            done < <(find "$src_wp_dir" -maxdepth 1 -type f \( -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.webp' -o -iname '*.bmp' -o -iname '*.avif' \) -print0 2>/dev/null)
            log "Espelhados $img_count wallpapers rotativos em $mirrored_wp_dir (0644 root:root)"
        fi
    fi

    esc_wp="$(toml_escape "${APPLIED_WALLPAPER:-$CFG_WALLPAPER}")"
    esc_wp_dir="$(toml_escape "${mirrored_wp_dir:-$CFG_WALLPAPER_DIR}")"
    esc_user="$(toml_escape "$CFG_DEFAULT_USER")"
    esc_session="$(toml_escape "$CFG_SESSION")"
    esc_accent="$(toml_escape "$CFG_ACCENT_COLOR")"

    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] Gravaria $STATE_DIR/greeter.toml, $STATE_DIR/env, $START_WRAPPER, $GREETD_CFG e $SYSTEMD_UNIT"
        return 0
    fi

    cat > "$STATE_DIR/greeter.toml" <<EOF
# Snapshot aplicado automaticamente por apply-greeter.sh
style = "$CFG_STYLE"
wallpaper = "$esc_wp"
wallpaper_mode = "$CFG_WALLPAPER_MODE"
wallpaper_dir = "$esc_wp_dir"
wallpaper_shuffle = $CFG_WALLPAPER_SHUFFLE
wallpaper_interval_mins = $CFG_WALLPAPER_INTERVAL_MINS
theme = "$CFG_THEME"
font_family = "$CFG_FONT_FAMILY"
font_weight = "$CFG_FONT_WEIGHT"
clock_size = "$CFG_CLOCK_SIZE"
clock_24h = $CFG_CLOCK_24H
show_seconds = $CFG_SHOW_SECONDS
show_date = $CFG_SHOW_DATE
show_avatar = $CFG_SHOW_AVATAR
show_username = $CFG_SHOW_USERNAME
blur_radius = $CFG_BLUR_RADIUS
dim_opacity = $CFG_DIM_OPACITY
card_opacity = $CFG_CARD_OPACITY
card_radius = $CFG_CARD_RADIUS
accent_color = "$esc_accent"
default_user = "$esc_user"
session = "$esc_session"
isolated_binary = $CFG_ISOLATED_BINARY
EOF
    chmod 0644 "$STATE_DIR/greeter.toml"

    cat > "$STATE_DIR/env" <<EOF
# Gerado por apply-greeter.sh — lido por /usr/local/bin/aenes-greeter-start
AENES_GREETER_BIN="$ACTIVE_GREETER_BIN"
AENES_GREETER_WALLPAPER="$APPLIED_WALLPAPER"
AENES_GREETER_CLOCK_24H="$CFG_CLOCK_24H"
AENES_GREETER_SHOW_SECONDS="$CFG_SHOW_SECONDS"
AENES_GREETER_BLUR_RADIUS="$CFG_BLUR_RADIUS"
AENES_GREETER_DIM_OPACITY="$CFG_DIM_OPACITY"
AENES_GREETER_THEME="$CFG_THEME"
AENES_GREETER_DEFAULT_USER="$CFG_DEFAULT_USER"
AENES_GREETER_SESSION="$CFG_SESSION"
EOF
    chmod 0644 "$STATE_DIR/env"

    cat > "$START_WRAPPER" <<'EOF'
#!/bin/sh
# Wrapper isolado de inicialização do greeter (não sobrescrito pelo pacman).
set -eu
if [ -r /var/lib/aenes-greeter/env ]; then
    set -a
    # shellcheck disable=SC1091
    . /var/lib/aenes-greeter/env
    set +a
fi
GREETER_BIN="${AENES_GREETER_BIN:-/usr/local/bin/aenes-greeter}"
if [ ! -x "$GREETER_BIN" ]; then
    GREETER_BIN="/usr/bin/cosmic-greeter"
fi
rm -rf /run/cosmic-greeter/cosmic/com.system76.CosmicSettingsDaemon/v1/* >/dev/null 2>&1 || true
exec cosmic-comp "$GREETER_BIN" >/dev/null 2>&1
EOF
    chmod 0755 "$START_WRAPPER"

    install -dm0755 /etc/greetd
    cat > "$GREETD_CFG" <<EOF
# Configuração greetd isolada (gerenciada por aenes-greeter / cosmic-taskbar)
# O arquivo oficial /etc/greetd/cosmic-greeter.toml permanece intocado.
[terminal]
vt = "1"

[general]
service = "login"

[default_session]
command = "$START_WRAPPER"
user = "$GREETER_USER"
EOF
    chmod 0644 "$GREETD_CFG"

    local greetd_bin
    greetd_bin="$(command -v greetd 2>/dev/null || echo /usr/bin/greetd)"
    cat > "$SYSTEMD_UNIT" <<EOF
[Unit]
Description=Aenes Isolated Login Greeter (greetd)
After=systemd-user-sessions.service plymouth-quit-wait.service cosmic-greeter-daemon.service
After=getty@tty1.service kmsconvt@tty1.service
Conflicts=getty@tty1.service kmsconvt@tty1.service cosmic-greeter.service
Wants=cosmic-greeter-daemon.service

[Service]
Type=simple
ExecStart=$greetd_bin --config $GREETD_CFG
IgnoreSIGPIPE=no
SendSIGHUP=yes
TimeoutStopSec=30s
KeyringMode=shared
Restart=always
RestartSec=1
StartLimitBurst=5
StartLimitInterval=30

[Install]
Alias=display-manager.service
EOF
    chmod 0644 "$SYSTEMD_UNIT"
}

enable_isolated_unit() {
    if [ "$DRY_RUN" = "true" ]; then
        log "[dry-run] systemctl daemon-reload && systemctl disable cosmic-greeter.service && systemctl enable aenes-greeter.service"
        return 0
    fi

    systemctl daemon-reload
    if systemctl is-enabled cosmic-greeter.service >/dev/null 2>&1; then
        systemctl disable cosmic-greeter.service
    fi
    # Nunca usamos --now aqui para não encerrar a sessão gráfica atual do usuário.
    systemctl enable --force aenes-greeter.service
    log "Unit aenes-greeter.service habilitada como display-manager.service (ativa no próximo login/boot)."
}

# --- Execução principal ---

case "$MODE" in
    status)
        show_status
        exit 0
        ;;
    restore)
        restore_official_greeter
        exit 0
        ;;
    install-only)
        require_root
        install_helper_and_polkit
        exit 0
        ;;
esac

require_root

TARGET_USER="$(resolve_target_user)"
TARGET_HOME="$(get_user_home "$TARGET_USER")"
[ -n "$CONFIG_FILE" ] || CONFIG_FILE="$TARGET_HOME/.config/cosmic-taskbar/greeter.toml"
if [ -z "$SOURCE_DIR" ]; then
    if [ -f "$SCRIPT_DIR/../../greeter/Cargo.toml" ]; then
        SOURCE_DIR="$(readlink -f "$SCRIPT_DIR/../../greeter")"
    elif [ -f "$TARGET_HOME/Projetos/cosmic-taskbar/greeter/Cargo.toml" ]; then
        SOURCE_DIR="$TARGET_HOME/Projetos/cosmic-taskbar/greeter"
    else
        SOURCE_DIR="$TARGET_HOME/Projetos/cosmic-greeter"
    fi
fi
GREETER_USER="$(detect_greeter_user)"
ACTIVE_GREETER_BIN="/usr/bin/cosmic-greeter"
APPLIED_WALLPAPER=""

if [ -f "$CONFIG_FILE" ]; then
    log "Lendo configuração de $CONFIG_FILE"
    parse_greeter_toml "$CONFIG_FILE"
else
    warn "Arquivo $CONFIG_FILE não encontrado; aplicando valores padrão."
fi

install_helper_and_polkit
backup_existing_state
ensure_isolated_binary "$SOURCE_DIR"
prepare_wallpaper "$TARGET_HOME"
write_greeter_cosmic_config
write_isolated_runtime_files
enable_isolated_unit

log "Configuração isolada aplicada com sucesso (binário: $ACTIVE_GREETER_BIN, tema: $CFG_THEME, sessão: $CFG_SESSION)."
