#!/bin/sh
# Bootstrap completo de um PC novo (Arch / CachyOS + COSMIC):
# - Instala pacotes (Brave, fish, print-recorte/satty, wallpapers-monitores, build Rust)
# - Configura fish como shell padrão + configs customizadas
# - Compila e instala cosmic-taskbar + extras (print, fix-brave-pwa-icons, wallpapers-monitores)
# - Aplica todas as configs visuais do COSMIC e atalhos
#
# Uso:
#   ./scripts/bootstrap-pc.sh
set -e
cd "$(dirname "$0")/.."

echo "[1/5] Instalando pacotes base via pacman..."
PKGS="fish grim slurp wl-clipboard satty libnotify python-pillow libvips rust"
if pacman -Si cachyos-fish-config >/dev/null 2>&1; then
    PKGS="$PKGS cachyos-fish-config"
fi
if pacman -Si brave-bin >/dev/null 2>&1; then
    PKGS="$PKGS brave-bin"
fi
sudo pacman -S --needed --noconfirm $PKGS

if ! command -v brave >/dev/null 2>&1 && ! pacman -Qi brave-bin >/dev/null 2>&1; then
    if command -v paru >/dev/null 2>&1; then
        paru -S --needed --noconfirm brave-bin
    elif command -v yay >/dev/null 2>&1; then
        yay -S --needed --noconfirm brave-bin
    else
        echo "Aviso: instale brave-bin pelo AUR manualmente (paru/yay não encontrado)."
    fi
fi

echo "[2/5] Configurando fish como shell padrão..."
FISH_BIN="$(command -v fish || true)"
if [ -n "$FISH_BIN" ] && [ "${SHELL:-}" != "$FISH_BIN" ]; then
    chsh -s "$FISH_BIN" || echo "Aviso: rode 'chsh -s $FISH_BIN' manualmente."
fi

echo "[3/5] Compilando e instalando cosmic-taskbar + utilitários visuais..."
./install.sh

echo "[4/5] Aplicando configurações visuais do COSMIC e do fish..."
./scripts/sync-config.sh apply

echo "[5/5] Aplicando atalhos e applets no painel..."
./setup.sh

echo "Tudo pronto! Reinicie a sessão (ou rode reiniciar-painel) para carregar tudo."
