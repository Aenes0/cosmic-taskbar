#!/bin/sh
# Swaps the panel's three corner applets (tray, Central, show desktop) for the
# one Canto applet, in a plugins_wings file. Sourced by setup.sh and by
# tests/setup_corner.sh.
#   corner_use FILE   -> the three, in a row, become the Canto
#   corner_undo FILE  -> the Canto becomes the three again
# Only the first matching run is touched; anything else is left as it is.

corner_tray=io.github.Aenes0.CosmicTray
corner_central=io.github.Aenes0.CosmicCentral
corner_show=io.github.Aenes0.CosmicCentral.ShowDesktop
corner_id=io.github.Aenes0.CosmicCentral.Corner

corner_use() {
    cw_file="$1"
    [ -f "$cw_file" ] || return 0
    if grep -q "\"$corner_id\"" "$cw_file"; then
        return 0
    fi
    cw_tmp="$cw_file.corner.$$"
    awk -v T="$corner_tray" -v C="$corner_central" -v S="$corner_show" -v K="$corner_id" '
        function item(s, id) {
            gsub(/^[ \t]+|[ \t]+$/, "", s)
            return s == "\"" id "\","
        }
        { line[NR] = $0 }
        END {
            found = 0
            for (i = 1; i <= NR - 2 && !found; i++) {
                if (item(line[i], T) && item(line[i + 1], C) && item(line[i + 2], S)) {
                    found = i
                }
            }
            for (j = 1; j <= NR; j++) {
                if (found && j == found) {
                    ind = line[j]
                    sub(/[^ \t].*$/, "", ind)
                    print ind "\"" K "\","
                } else if (found && j > found && j <= found + 2) {
                    continue
                } else {
                    print line[j]
                }
            }
        }' "$cw_file" > "$cw_tmp"
    if cmp -s "$cw_file" "$cw_tmp"; then
        rm -f "$cw_tmp"
        if grep -q "\"$corner_tray\"\|\"$corner_central\"\|\"$corner_show\"" "$cw_file"; then
            echo "Canto: the corner's three applets are not in a row on the panel, left as they are. Add \"Canto\" in Settings > Desktop > Panel > Applets."
        fi
        return 0
    fi
    mv "$cw_tmp" "$cw_file"
}

corner_undo() {
    cw_file="$1"
    [ -f "$cw_file" ] || return 0
    grep -q "\"$corner_id\"" "$cw_file" || return 0
    cw_tmp="$cw_file.corner.$$"
    awk -v T="$corner_tray" -v C="$corner_central" -v S="$corner_show" -v K="$corner_id" '
        function item(s, id) {
            gsub(/^[ \t]+|[ \t]+$/, "", s)
            return s == "\"" id "\","
        }
        {
            if (item($0, K)) {
                ind = $0
                sub(/[^ \t].*$/, "", ind)
                print ind "\"" T "\","
                print ind "\"" C "\","
                print ind "\"" S "\","
            } else {
                print
            }
        }' "$cw_file" > "$cw_tmp"
    if cmp -s "$cw_file" "$cw_tmp"; then
        rm -f "$cw_tmp"
        return 0
    fi
    mv "$cw_tmp" "$cw_file"
}
