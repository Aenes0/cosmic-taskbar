#!/bin/sh
# Abre o locker numa janela (cosmic-comp aninhado), sem tocar na sessão real.
# Grava pidfiles em target/dev/ para o stop-nested.sh e o socket do compositor aninhado em nested.display.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
bin=$(realpath "${1:-$repo/target/release/cosmic-greeter}")
dev="$repo/target/dev"
inner="$dev/nested-locker-inner.sh"
# Serviço PAM: login por padrão; COSMIC_GREETER_PAM_SERVICE= (vazio) usa o padrão compilado no binário.
pam="${COSMIC_GREETER_PAM_SERVICE-login}"
# Medição (opcional): repassa estas variáveis ao cliente aninhado.
perfenv=""
[ -n "${COSMIC_GREETER_PERF_LOG-}" ] && perfenv="export COSMIC_GREETER_PERF_LOG='$COSMIC_GREETER_PERF_LOG'"
[ -n "${COSMIC_GREETER_MOTION_DEMO-}" ] && perfenv="$perfenv
export COSMIC_GREETER_MOTION_DEMO='$COSMIC_GREETER_MOTION_DEMO'"
# Config de estilo alternativa (usada por prints-estilos.sh): repassada ao cliente aninhado.
[ -n "${AENES_GREETER_CONFIG-}" ] && perfenv="$perfenv
export AENES_GREETER_CONFIG='$AENES_GREETER_CONFIG'"
mkdir -p "$dev"
oldpid=$(cat "$dev/comp.pid" 2>/dev/null || true)
case "$oldpid" in ''|*[!0-9]*) oldpid= ;; esac
if [ -n "$oldpid" ] && [ -r "/proc/$oldpid/cmdline" ] && tr '\0' ' ' < "/proc/$oldpid/cmdline" | grep -q "$dev/nested-"; then
  echo "já há um compositor aninhado rodando (pid $oldpid): rode scripts/dev/stop-nested.sh antes" >&2
  exit 1
fi
rm -f "$dev"/*.pid "$dev/nested.display"
cat > "$inner" <<INNER
#!/bin/sh
echo "\$WAYLAND_DISPLAY" > "$dev/nested.display"
cosmic-bg &
echo \$! > "$dev/bg.pid"
sleep 1
echo \$\$ > "$dev/client.pid"
$perfenv
date +%s%3N > "$dev/launch.ms"
COSMIC_GREETER_NESTED=1 ${pam:+COSMIC_GREETER_PAM_SERVICE=$pam} RUST_LOG=cosmic_greeter=info exec "$bin"
INNER
chmod +x "$inner"
# winit: o backend X11 (padrão com DISPLAY) pisca na NVIDIA via Xwayland; winit não.
COSMIC_BACKEND=winit cosmic-comp "$inner" &
echo $! > "$dev/comp.pid"
wait
