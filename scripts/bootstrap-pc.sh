#!/bin/sh
# Bootstrap completo de um PC novo (Arch / CachyOS + COSMIC):
# - Instala TODAS as dependências de build e runtime auditadas do ambiente
# - Instala Brave + fish (+ cachyos-fish-config + fcitx5)
# - Instala udev rule do Logitech (`70-logi-osd.rules`)
# - Compila e instala cosmic-taskbar + utilitários visuais (print, wallpapers, PWA)
# - Aplica todas as configs visuais do COSMIC e do fish + atalhos
#
# Uso:
#   ./scripts/bootstrap-pc.sh
set -e
cd "$(dirname "$0")/.."

echo "[1/6] Instalando pacotes base e dependências de build/runtime..."
# Build Rust + libs nativas (libudev-sys, wayland, libxkbcommon)
BUILD_PKGS="base-devel pkgconf rust systemd-libs libxkbcommon wayland"
# Runtime Central / Monitor / Audio / Clima / Mídia
CENTRAL_PKGS="curl wireplumber pipewire libpulse xdg-utils xdg-user-dirs"
# Runtime print-recorte + print-preview
PRINT_PKGS="grim slurp wl-clipboard satty libnotify python-gobject gtk4 gtk4-layer-shell"
# Runtime wallpapers-monitores
WALLPAPER_PKGS="python python-pillow libvips"
# Shell + acentos/cedilha
SHELL_PKGS="fish fcitx5"

PKGS="$BUILD_PKGS $CENTRAL_PKGS $PRINT_PKGS $WALLPAPER_PKGS $SHELL_PKGS"
if pacman -Si cachyos-fish-config >/dev/null 2>&1; then
    PKGS="$PKGS cachyos-fish-config"
fi
if pacman -Si brave-bin >/dev/null 2>&1; then
    PKGS="$PKGS brave-bin"
fi
if pacman -Si mission-center >/dev/null 2>&1; then
    PKGS="$PKGS mission-center"
fi
sudo pacman -S --needed --noconfirm $PKGS

if ! command -v missioncenter >/dev/null 2>&1 && ! pacman -Qi mission-center >/dev/null 2>&1; then
    if command -v flatpak >/dev/null 2>&1; then
        flatpak install -y flathub io.missioncenter.MissionCenter || true
    fi
fi

if ! command -v brave >/dev/null 2>&1 && ! pacman -Qi brave-bin >/dev/null 2>&1; then
    if command -v paru >/dev/null 2>&1; then
        paru -S --needed --noconfirm brave-bin
    elif command -v yay >/dev/null 2>&1; then
        yay -S --needed --noconfirm brave-bin
    else
        echo "Aviso: instale brave-bin pelo AUR manualmente (paru/yay não encontrado)."
    fi
fi

echo "[2/6] Regra udev para dispositivos Logitech (logi-osd)..."
if [ ! -e /etc/udev/rules.d/70-logi-osd.rules ] && [ ! -e /usr/lib/udev/rules.d/42-logitech-unify-permissions.rules ]; then
    sudo install -Dm0644 data/logi/70-logi-osd.rules /etc/udev/rules.d/70-logi-osd.rules
    sudo udevadm control --reload
    sudo udevadm trigger --subsystem-match=hidraw
fi

echo "[3/6] Configurando fish como shell padrão..."
FISH_BIN="$(command -v fish || true)"
if [ -n "$FISH_BIN" ] && [ "${SHELL:-}" != "$FISH_BIN" ]; then
    chsh -s "$FISH_BIN" || echo "Aviso: rode 'chsh -s $FISH_BIN' manualmente."
fi

echo "[4/6] Compilando e instalando cosmic-taskbar + utilitários visuais..."
./install.sh

echo "[5/6] Aplicando configurações visuais do COSMIC e do fish..."
./scripts/sync-config.sh apply

echo "[6/6] Aplicando atalhos e applets no painel..."
./setup.sh

echo "Tudo pronto! Reinicie a sessão (ou rode reiniciar-painel) para carregar tudo."
