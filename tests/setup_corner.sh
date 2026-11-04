#!/bin/sh
# Tests the Canto swap of the panel's wings (scripts/corner_wings.sh) on a
# temporary copy of plugins_wings; never touches the user's panel.
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
. "$root/scripts/corner_wings.sh"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
f="$tmp/plugins_wings"
orig="$tmp/original"
fail() { echo "FALHA: $1"; exit 1; }

# The real content, plus the Monitor in another list (it must not be touched).
cat > "$f" <<'EOT'
[
    "io.github.Aenes0.CosmicTray",
    "io.github.Aenes0.CosmicCentral",
    "io.github.Aenes0.CosmicCentral.ShowDesktop",
    "com.system76.CosmicPanelWorkspacesButton",
]
EOT
cat >> "$f" <<'EOT'
[
    "io.github.Aenes0.CosmicCentral.Monitor",
]
EOT
cp "$f" "$orig"

# 1. ida troca as 3 linhas por uma
corner_use "$f"
grep -q '^    "io.github.Aenes0.CosmicCentral.Corner",$' "$f" || fail "ida nao inseriu o Corner com a indentacao"
grep -q '"io.github.Aenes0.CosmicTray"' "$f" && fail "ida nao removeu o Tray"
grep -q '"io.github.Aenes0.CosmicCentral",' "$f" && fail "ida nao removeu o Central"
grep -q 'CosmicCentral.ShowDesktop' "$f" && fail "ida nao removeu o ShowDesktop"
grep -q 'CosmicCentral.Monitor' "$f" || fail "ida mexeu no Monitor"
grep -q 'CosmicPanelWorkspacesButton' "$f" || fail "ida mexeu nas outras linhas"
cp "$f" "$tmp/ida1"
echo "ok ida troca"

# 2. segunda ida nao muda nada
corner_use "$f"
cmp -s "$f" "$tmp/ida1" || fail "segunda ida mudou o arquivo"
echo "ok segunda ida nao muda"

# 3. --undo volta ao original
corner_undo "$f"
cmp -s "$f" "$orig" || fail "undo nao voltou ao original"
echo "ok undo volta ao original"

# 4. arquivo sem os 3 seguidos nao muda (e sem Corner)
cat > "$f" <<'EOT'
[
    "io.github.Aenes0.CosmicTray",
    "com.system76.CosmicPanelWorkspacesButton",
    "io.github.Aenes0.CosmicCentral.ShowDesktop",
]
EOT
cp "$f" "$tmp/parcial"
corner_use "$f" >/dev/null
cmp -s "$f" "$tmp/parcial" || fail "arquivo sem os 3 seguidos foi alterado"
corner_undo "$f"
cmp -s "$f" "$tmp/parcial" || fail "undo alterou arquivo sem o Corner"
echo "ok sem os 3 seguidos nao muda"

# 5. arquivo inexistente nao falha
corner_use "$tmp/nao-existe" && corner_undo "$tmp/nao-existe"
echo "OK"
