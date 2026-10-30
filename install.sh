#!/bin/sh
# Build and install for the current user (default) or under another prefix:
#   ./install.sh            -> ~/.local
#   sudo ./install.sh /usr  -> system wide
set -e
cd "$(dirname "$0")"
prefix="${1:-$HOME/.local}"
bin=cosmic-taskbar

# The CPU renderer is patched (see third_party/libcosmic-patches).
bash scripts/libcosmic-patched.sh
cargo build --release
install -Dm0755 "target/release/$bin" "$prefix/bin/$bin"

# The two programs this one replaces were called by these names, by the
# shortcuts (Super, Super+X) and panel entries they set up: the old names
# stay as links, and the program takes the part its name says.
for old in cosmic-ext-applet-taskbar cosmic-ext-applet-start; do
    ln -sf "$bin" "$prefix/bin/$old"
done

mkdir -p "$prefix/share/applications"
for id in io.github.Aenes0.CosmicTaskbar io.github.Aenes0.CosmicStart; do
    sed "s|@bindir@|$prefix/bin|" "data/$id.desktop.in" > "$prefix/share/applications/$id.desktop"
done
echo "Installed. Run ./setup.sh to use the taskbar and the Start menu in the panel."
