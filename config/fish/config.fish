source /usr/share/cachyos-fish-config/cachyos-config.fish

# overwrite greeting
# potentially disabling fastfetch
function fish_greeting
end


# Added by Antigravity CLI installer
set -gx PATH "@HOME@/.local/bin" $PATH
set -gx NODE_COMPILE_CACHE "@HOME@/.cache/node-compile-cache"
