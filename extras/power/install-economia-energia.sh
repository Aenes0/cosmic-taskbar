#!/usr/bin/env bash
# Instala (ou remove) o daemon economia-energia e as regras de hardware/energia
# do cosmic-taskbar (áudio power_save, RAPL, i2c-dev, nct6775, hidraw Logitech).
#
# Arquivos gerenciados (100% fora de /usr/bin):
#   - /usr/local/bin/economia-energia
#   - /usr/local/bin/economia-energia-daemon
#   - /usr/local/bin/economia-energia-ventoinhas
#   - /etc/systemd/system/economia-energia.service
#   - /etc/modprobe.d/99-economia-energia.conf
#   - /etc/modules-load.d/i2c-dev.conf
#   - /etc/modules-load.d/nct6775.conf
#   - /etc/udev/rules.d/99-rapl.rules
#   - /etc/udev/rules.d/70-logi-osd.rules
#
# Uso:
#   sudo ./extras/power/install-economia-energia.sh
#   sudo ./extras/power/install-economia-energia.sh --uninstall
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    echo "[economia-energia] ERRO: execute como root (ex.: sudo $0)." >&2
    exit 1
fi

if [ "${1:-}" = "--uninstall" ]; then
    systemctl disable --now economia-energia.service 2>/dev/null || true
    rm -f /etc/systemd/system/economia-energia.service \
          /usr/local/bin/economia-energia \
          /usr/local/bin/economia-energia-daemon \
          /usr/local/bin/economia-energia-ventoinhas \
          /etc/modprobe.d/99-economia-energia.conf
    systemctl daemon-reload
    echo "[economia-energia] Removido."
    exit 0
fi

install -Dm0755 "$SCRIPT_DIR/economia-energia" /usr/local/bin/economia-energia
install -Dm0755 "$SCRIPT_DIR/economia-energia-daemon" /usr/local/bin/economia-energia-daemon
install -Dm0755 "$SCRIPT_DIR/economia-energia-ventoinhas" /usr/local/bin/economia-energia-ventoinhas
install -Dm0644 "$SCRIPT_DIR/economia-energia.service" /etc/systemd/system/economia-energia.service

# Economia de energia de áudio no boot (snd_hda_intel)
install -dm0755 /etc/modprobe.d
printf 'options snd_hda_intel power_save=1 power_save_controller=Y\n' > /etc/modprobe.d/99-economia-energia.conf
chmod 0644 /etc/modprobe.d/99-economia-energia.conf

# Módulos para brilho DDC/CI (i2c-dev) e sensores/ventoinhas da placa-mãe (nct6775)
install -dm0755 /etc/modules-load.d
printf 'i2c-dev\n' > /etc/modules-load.d/i2c-dev.conf
printf 'nct6775\n' > /etc/modules-load.d/nct6775.conf
chmod 0644 /etc/modules-load.d/i2c-dev.conf /etc/modules-load.d/nct6775.conf

# Regras udev: leitura RAPL (consumo em Watts no Monitor) e hidraw Logitech
install -dm0755 /etc/udev/rules.d
printf 'SUBSYSTEM=="powercap", KERNEL=="intel-rapl*", RUN+="/usr/bin/chmod a+r /sys/%%p/energy_uj"\n' > /etc/udev/rules.d/99-rapl.rules
chmod 0644 /etc/udev/rules.d/99-rapl.rules
if [ -f "$REPO_ROOT/data/logi/70-logi-osd.rules" ]; then
    install -Dm0644 "$REPO_ROOT/data/logi/70-logi-osd.rules" /etc/udev/rules.d/70-logi-osd.rules
fi

udevadm control --reload 2>/dev/null || true
udevadm trigger --subsystem-match=powercap --subsystem-match=hidraw 2>/dev/null || true

systemctl daemon-reload
systemctl enable --now economia-energia.service
echo "[economia-energia] Instalado e ativo (daemon + modprobe + módulos + regras udev)."
