#!/bin/sh
# Encerra só o que nested-*.sh abriu: clientes primeiro (senão giram em "Broken pipe"), depois o
# compositor aninhado. Usa pidfiles de target/dev/ e, de reserva, o caminho exato dos binários do repo
# (regex ancorada no início da linha de comando: não casa com `sh -c` de quem chama).
# Um pidfile só vale se /proc/<pid> ainda for o nosso processo (PID reaproveitado = pidfile velho:
# apenas apagado). Nunca use pkill cosmic-comp (mata a sessão real).
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
dev="$repo/target/dev"
bins="$repo/target/release/(cosmic-greeter|examples/server)( |\$)"
# O Painel (nested-panel.sh) mora no target do repo pai: caminho exato, também ancorado.
panel_bin="$(cd "$repo/.." && pwd)/target/release/cosmic-taskbar"
panel_re="^$panel_bin --control-panel( |\$)"

cmdline() { [ -r "/proc/$1/cmdline" ] && tr '\0' ' ' < "/proc/$1/cmdline" || true; }

# is_ours KIND PID: o PID ainda é o processo que nested-*.sh gravou?
is_ours() {
  cmd=$(cmdline "$2")
  case "$1" in
    client) case "$cmd" in "$repo/target/"*|"$panel_bin --control-panel"*) return 0 ;; esac ;;
    comp) case "$cmd" in "cosmic-comp $dev/nested-"*|*"/cosmic-comp $dev/nested-"*) return 0 ;; esac ;;
    bg) # cosmic-bg rodando no WAYLAND_DISPLAY do compositor aninhado (não o da sessão real)
      disp=$(cat "$dev/nested.display" 2>/dev/null || true)
      case "$cmd" in cosmic-bg*|*/cosmic-bg*) ;; *) return 1 ;; esac
      [ -n "$disp" ] && [ -r "/proc/$2/environ" ] && tr '\0' '\n' < "/proc/$2/environ" | grep -qx "WAYLAND_DISPLAY=$disp" && return 0 ;;
  esac
  return 1
}

# signal_pidfile KIND SIGNAL: sinaliza só se for nosso; pidfile velho é ignorado.
signal_pidfile() {
  [ -f "$dev/$1.pid" ] || return 0
  pid=$(cat "$dev/$1.pid")
  case "$pid" in ''|*[!0-9]*) return 0 ;; esac
  if is_ours "$1" "$pid"; then kill "-$2" "$pid" 2>/dev/null || true; fi
}

signal_pidfile client TERM
signal_pidfile bg TERM
pkill -f "^$bins" || true
pkill -f "$panel_re" || true
sleep 1
signal_pidfile comp TERM
sleep 1
# Reserva: quem ainda estiver vivo (e ainda for nosso) leva SIGKILL.
pkill -9 -f "^$bins" || true
pkill -9 -f "$panel_re" || true
for k in client bg comp; do signal_pidfile "$k" KILL; done
rm -f "$dev"/*.pid "$dev/nested.display" "$dev/socket"
