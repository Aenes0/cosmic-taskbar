#!/usr/bin/env bash
# Instala (ou remove) o daemon economia-energia a partir do cosmic-taskbar.
#
# Arquivos gerenciados (isolados em /usr/local/bin e /etc/systemd/system):
#   - /usr/local/bin/economia-energia
#   - /usr/local/bin/economia-energia-daemon
#   - /usr/local/bin/economia-energia-ventoinhas
#   - /etc/systemd/system/economia-energia.service
#
# Uso:
#   sudo ./extras/power/install-economia-energia.sh
#   sudo ./extras/power/install-economia-energia.sh --uninstall
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    echo "[economia-energia] ERRO: execute como root (ex.: sudo $0)." >&2
    exit 1
fi

if [ "${1:-}" = "--uninstall" ]; then
    systemctl disable --now economia-energia.service 2>/dev/null || true
    rm -f /etc/systemd/system/economia-energia.service \
          /usr/local/bin/economia-energia \
          /usr/local/bin/economia-energia-daemon \
          /usr/local/bin/economia-energia-ventoinhas
    systemctl daemon-reload
    echo "[economia-energia] Removido."
    exit 0
fi

install -Dm0755 "$SCRIPT_DIR/economia-energia" /usr/local/bin/economia-energia
install -Dm0755 "$SCRIPT_DIR/economia-energia-daemon" /usr/local/bin/economia-energia-daemon
install -Dm0755 "$SCRIPT_DIR/economia-energia-ventoinhas" /usr/local/bin/economia-energia-ventoinhas
install -Dm0644 "$SCRIPT_DIR/economia-energia.service" /etc/systemd/system/economia-energia.service

systemctl daemon-reload
systemctl enable --now economia-energia.service
echo "[economia-energia] Instalado e ativo (economia-energia.service)."
