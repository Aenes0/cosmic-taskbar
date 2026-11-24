#!/bin/sh
# Abre a seção Login do Painel (cosmic-taskbar --control-panel --section login) num cosmic-comp aninhado,
# lendo um greeter.toml isolado (XDG_CONFIG_HOME temporário). Não toca a config nem a sessão reais.
# Uso: nested-panel.sh <style> <theme>   (ex.: nested-panel.sh win11 dark)
# Grava pidfiles em target/dev/ para o stop-nested.sh e o socket do compositor aninhado em nested.display.
set -eu
[ $# -eq 2 ] || { echo "uso: nested-panel.sh <style> <theme>" >&2; exit 2; }
style=$1
theme=$2
case "$style$theme" in *[!A-Za-z0-9_-]*) echo "style/theme: só letras, dígitos, _ e -" >&2; exit 2 ;; esac
repo=$(cd "$(dirname "$0")/../.." && pwd)
bin="$(cd "$repo/.." && pwd)/target/release/cosmic-taskbar"
[ -x "$bin" ] || { echo "$bin não existe: compile o Painel (cargo build --release na raiz)" >&2; exit 1; }
dev="$repo/target/dev"
xdg="$dev/xdg-$style-$theme"
inner="$dev/nested-panel-inner.sh"
mkdir -p "$dev" "$xdg/cosmic-taskbar"
oldpid=$(cat "$dev/comp.pid" 2>/dev/null || true)
case "$oldpid" in ''|*[!0-9]*) oldpid= ;; esac
if [ -n "$oldpid" ] && [ -r "/proc/$oldpid/cmdline" ] && tr '\0' ' ' < "/proc/$oldpid/cmdline" | grep -q "$dev/nested-"; then
  echo "já há um compositor aninhado rodando (pid $oldpid): rode scripts/dev/stop-nested.sh antes" >&2
  exit 1
fi
# greeter.toml isolado: parte do exemplo do repo, trocando só style/theme (no topo, antes de qualquer tabela).
src="$repo/../extras/greeter/greeter.toml.example"
cfg="$xdg/cosmic-taskbar/greeter.toml"
{
  printf 'style = "%s"\ntheme = "%s"\n' "$style" "$theme"
  [ -f "$src" ] && grep -Ev '^[[:space:]]*(style|theme)[[:space:]]*=' "$src" || true
} > "$cfg"
rm -f "$dev"/*.pid "$dev/nested.display"
cat > "$inner" <<INNER
#!/bin/sh
echo "\$WAYLAND_DISPLAY" > "$dev/nested.display"
cosmic-bg &
echo \$! > "$dev/bg.pid"
sleep 1
echo \$\$ > "$dev/client.pid"
export XDG_CONFIG_HOME='$xdg'
exec "$bin" --control-panel --section login
INNER
chmod +x "$inner"
COSMIC_BACKEND=winit cosmic-comp "$inner" &
echo $! > "$dev/comp.pid"
wait
