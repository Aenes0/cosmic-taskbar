#!/bin/sh
# Tira um print de cada estilo x tema do locker (ou do greeter) no compositor aninhado.
# Uso: prints-estilos.sh [locker|greeter]   (padrão: locker)
# Parte do ~/.config/cosmic-taskbar/greeter.toml do usuário (ou do exemplo do repo), trocando só style/theme.
# Saída: target/prints/<modo>-<style>-<theme>.png. Nunca digita senha.
set -eu
modo=${1:-locker}
case "$modo" in locker|greeter) ;; *) echo "uso: prints-estilos.sh [locker|greeter]" >&2; exit 2 ;; esac
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
dev="$repo/target/dev"
cfgdir="$dev/cfg"
mkdir -p "$cfgdir"
src="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic-taskbar/greeter.toml"
[ -f "$src" ] || src="$repo/../extras/greeter/greeter.toml.example"
[ -f "$src" ] || src=/dev/null
for style in win11 android macos ios ipados; do
  for theme in dark light; do
    cfg="$cfgdir/$style-$theme.toml"
    {
      printf 'style = "%s"\ntheme = "%s"\n' "$style" "$theme"
      grep -Ev '^[[:space:]]*(style|theme)[[:space:]]*=' "$src" || true
    } > "$cfg"
    "$here/stop-nested.sh" >/dev/null 2>&1 || true
    AENES_GREETER_CONFIG="$cfg" "$here/nested-$modo.sh" >/dev/null 2>&1 &
    # espera o compositor aninhado publicar o display (até 30 s)
    n=0
    while [ ! -s "$dev/nested.display" ] && [ "$n" -lt 60 ]; do sleep 0.5; n=$((n + 1)); done
    if [ ! -s "$dev/nested.display" ]; then
      echo "falhou: $modo-$style-$theme (compositor aninhado não subiu)" >&2
      "$here/stop-nested.sh" >/dev/null 2>&1 || true
      continue
    fi
    sleep 3
    python3 "$here/print.py" "$modo-$style-$theme" || echo "falhou: print $modo-$style-$theme" >&2
    "$here/stop-nested.sh"
    wait || true
  done
done
