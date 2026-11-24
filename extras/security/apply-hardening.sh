#!/usr/bin/env bash
# Hardening de privacidade e segurança do Linux gerenciado pelo cosmic-taskbar.
# 100% isolado em /etc/sysctl.d/, /etc/NetworkManager/conf.d/ e /etc/systemd/resolved.conf.d/
# (nunca modifica arquivos de pacotes em /usr/).
#
# Uso:
#   sudo ./extras/security/apply-hardening.sh --kernel
#   sudo ./extras/security/apply-hardening.sh --kernel-undo
#   sudo ./extras/security/apply-hardening.sh --nm-privacy
#   sudo ./extras/security/apply-hardening.sh --nm-privacy-undo
#   sudo ./extras/security/apply-hardening.sh --dns cloudflare|quad9|mullvad|dhcp
#   sudo ./extras/security/apply-hardening.sh --firewall-strict
set -euo pipefail

SYSCTL_FILE="/etc/sysctl.d/99-aenes-privacy.conf"
ARMOR_SYSCTL_FILE="/etc/sysctl.d/99-aenes-kernel-armor.conf"
BLACKLIST_FILE="/etc/modprobe.d/99-aenes-blacklist.conf"
COREDUMP_FILE="/etc/systemd/coredump.conf.d/99-aenes-nocoredump.conf"
NM_FILE="/etc/NetworkManager/conf.d/99-aenes-privacy.conf"
DNS_FILE="/etc/systemd/resolved.conf.d/99-aenes-dns.conf"

if [ "${EUID:-$(id -u)}" -ne 0 ]; then
    echo "[aenes-security] ERRO: execute como root (ex.: sudo $0 --kernel)." >&2
    exit 1
fi

apply_kernel() {
    install -dm0755 /etc/sysctl.d
    cat > "$SYSCTL_FILE" <<'EOF'
# Hardening de privacidade & rede — cosmic-taskbar (aenes)
# Kernel pointers, dmesg, ptrace & eBPF
kernel.kptr_restrict = 2
kernel.dmesg_restrict = 1
kernel.yama.ptrace_scope = 2
kernel.unprivileged_bpf_disabled = 2
net.core.bpf_jit_harden = 2

# Proteção de links no filesystem
fs.protected_hardlinks = 1
fs.protected_symlinks = 1
fs.suid_dumpable = 0

# Privacidade & proteção de rede (IPv4 / IPv6)
net.ipv4.tcp_syncookies = 1
net.ipv4.conf.all.rp_filter = 1
net.ipv4.conf.default.rp_filter = 1
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.default.accept_redirects = 0
net.ipv4.conf.all.send_redirects = 0
net.ipv4.conf.default.send_redirects = 0
net.ipv4.icmp_echo_ignore_broadcasts = 1
net.ipv4.icmp_echo_ignore_all = 1
net.ipv6.conf.all.accept_redirects = 0
net.ipv6.conf.default.accept_redirects = 0
net.ipv6.conf.all.use_tempaddr = 2
net.ipv6.conf.default.use_tempaddr = 2
EOF
    chmod 0644 "$SYSCTL_FILE"
    sysctl --system >/dev/null
    echo "[aenes-security] Hardening de kernel aplicado em $SYSCTL_FILE."
}

undo_kernel() {
    rm -f "$SYSCTL_FILE"
    sysctl -w net.ipv4.icmp_echo_ignore_all=0 net.ipv6.conf.all.use_tempaddr=0 kernel.yama.ptrace_scope=1 >/dev/null 2>&1 || true
    sysctl --system >/dev/null
    echo "[aenes-security] Hardening de kernel revertido."
}

apply_nm_privacy() {
    install -dm0755 /etc/NetworkManager/conf.d
    cat > "$NM_FILE" <<'EOF'
# Privacidade de rede — cosmic-taskbar (aenes)
# Desativa pings de telemetria (connectivity check), randomiza MAC em scans Wi-Fi
# e ativa endereços IPv6 temporários (RFC 4941).
[connectivity]
enabled=false

[device]
wifi.scan-rand-mac-address=yes

[connection]
wifi.cloned-mac-address=stable
ethernet.cloned-mac-address=stable
ipv6.ip6-privacy=2
EOF
    chmod 0644 "$NM_FILE"
    systemctl reload NetworkManager.service 2>/dev/null || true
    echo "[aenes-security] Privacidade do NetworkManager aplicada em $NM_FILE."
}

undo_nm_privacy() {
    rm -f "$NM_FILE"
    systemctl reload NetworkManager.service 2>/dev/null || true
    echo "[aenes-security] Privacidade do NetworkManager removida."
}

apply_dns() {
    local provider="${1:-quad9}"
    install -dm0755 /etc/systemd/resolved.conf.d
    case "$provider" in
        cloudflare)
            cat > "$DNS_FILE" <<'EOF'
[Resolve]
DNS=1.1.1.1#cloudflare-dns.com 1.0.0.1#cloudflare-dns.com 2606:4700:4700::1111#cloudflare-dns.com
FallbackDNS=9.9.9.9#dns.quad9.net
DNSOverTLS=yes
LLMNR=no
MulticastDNS=no
EOF
            ;;
        quad9)
            cat > "$DNS_FILE" <<'EOF'
[Resolve]
DNS=9.9.9.9#dns.quad9.net 149.112.112.112#dns.quad9.net 2620:fe::fe#dns.quad9.net
FallbackDNS=1.1.1.1#cloudflare-dns.com
DNSOverTLS=yes
LLMNR=no
MulticastDNS=no
EOF
            ;;
        mullvad)
            cat > "$DNS_FILE" <<'EOF'
[Resolve]
DNS=194.242.2.2#dns.mullvad.net 2a07:e340::2#dns.mullvad.net
FallbackDNS=9.9.9.9#dns.quad9.net
DNSOverTLS=yes
LLMNR=no
MulticastDNS=no
EOF
            ;;
        dhcp)
            rm -f "$DNS_FILE"
            ;;
        *)
            echo "[aenes-security] Provedor desconhecido: $provider (use cloudflare, quad9, mullvad ou dhcp)" >&2
            exit 1
            ;;
    esac
    [ -f "$DNS_FILE" ] && chmod 0644 "$DNS_FILE"
    systemctl restart systemd-resolved.service
    resolvectl status | head -n 12
}

apply_firewall_strict() {
    if ! command -v ufw >/dev/null 2>&1; then
        echo "[aenes-security] ERRO: ufw não está instalado (instale com: sudo pacman -S ufw)." >&2
        exit 1
    fi
    ufw default deny incoming
    ufw default allow outgoing
    ufw --force enable
    systemctl enable --now ufw.service
    ufw status verbose
}

apply_kernel_armor() {
    install -dm0755 /etc/sysctl.d /etc/modprobe.d /etc/systemd/coredump.conf.d
    cat > "$ARMOR_SYSCTL_FILE" <<'EOF'
# Blindagem avançada do núcleo Linux — cosmic-taskbar (aenes)
# Impede substituição do kernel em RAM (kexec), bloqueia SysRq, restringe
# perf_event_open contra side-channels, restringe io_uring sem privilégio,
# desativa core dumps SUID e mantém ASLR máximo + mmap_min_addr.
kernel.kexec_load_disabled = 1
kernel.sysrq = 0
kernel.perf_event_paranoid = 3
kernel.randomize_va_space = 2
kernel.io_uring_disabled = 1
vm.mmap_min_addr = 65536
fs.suid_dumpable = 0
fs.protected_fifos = 2
fs.protected_regular = 2
dev.tty.ldisc_autoload = 0
EOF
    chmod 0644 "$ARMOR_SYSCTL_FILE"

    # Desativa gravação de coredumps em disco (evita vazar senhas/chaves da RAM)
    cat > "$COREDUMP_FILE" <<'EOF'
[Coredump]
Storage=none
ProcessSizeMax=0
EOF
    chmod 0644 "$COREDUMP_FILE"

    # Blacklist de protocolos de rede e filesystems legados (superfície de exploits do kernel)
    cat > "$BLACKLIST_FILE" <<'EOF'
# Protocolos de rede legados raramente usados em desktop
install dccp /bin/false
install sctp /bin/false
install rds /bin/false
install tipc /bin/false
# Filesystems legados com histórico de vulnerabilidades
install cramfs /bin/false
install freevxfs /bin/false
install jffs2 /bin/false
install hfs /bin/false
install hfsplus /bin/false
# FireWire (risco de acesso DMA)
install firewire-core /bin/false
install firewire-ohci /bin/false
EOF
    chmod 0644 "$BLACKLIST_FILE"

    sysctl --system >/dev/null 2>&1 || true
    echo "[aenes-security] Blindagem avançada do núcleo aplicada (sysctl + nocoredump + blacklist de módulos)."
}

undo_kernel_armor() {
    rm -f "$ARMOR_SYSCTL_FILE" "$COREDUMP_FILE" "$BLACKLIST_FILE"
    sysctl -w kernel.sysrq=1 kernel.perf_event_paranoid=2 kernel.io_uring_disabled=0 >/dev/null 2>&1 || true
    sysctl --system >/dev/null 2>&1 || true
    echo "[aenes-security] Blindagem avançada revertida (kexec_load_disabled só volta após reboot)."
}

toggle_sysctl_dropin() {
    local name="$1" key="$2" on_val="$3" off_val="$4" mode="$5"
    local file="/etc/sysctl.d/99-aenes-opt-${name}.conf"
    install -dm0755 /etc/sysctl.d
    if [ "$mode" = "on" ]; then
        printf '# Toggle individual — cosmic-taskbar\n%s = %s\n' "$key" "$on_val" > "$file"
        chmod 0644 "$file"
        sysctl -w "${key}=${on_val}" >/dev/null 2>&1 || true
        echo "[aenes-security] ${key} = ${on_val} ativado."
    else
        rm -f "$file"
        sysctl -w "${key}=${off_val}" >/dev/null 2>&1 || true
        echo "[aenes-security] ${key} revertido para ${off_val}."
    fi
}

toggle_nocoredump() {
    local mode="${1:-on}"
    if [ "$mode" = "on" ]; then
        install -dm0755 /etc/systemd/coredump.conf.d
        printf '[Coredump]\nStorage=none\nProcessSizeMax=0\n' > "$COREDUMP_FILE"
        chmod 0644 "$COREDUMP_FILE"
        toggle_sysctl_dropin suid-dumpable fs.suid_dumpable 0 2 on
        echo "[aenes-security] Coredumps bloqueados (Storage=none)."
    else
        rm -f "$COREDUMP_FILE"
        toggle_sysctl_dropin suid-dumpable fs.suid_dumpable 0 2 off
        echo "[aenes-security] Coredumps reabilitados."
    fi
}

toggle_blacklist() {
    local mode="${1:-on}"
    if [ "$mode" = "on" ]; then
        install -dm0755 /etc/modprobe.d
        cat > "$BLACKLIST_FILE" <<'EOF'
# Protocolos de rede e filesystems legados — cosmic-taskbar
install dccp /bin/false
install sctp /bin/false
install rds /bin/false
install tipc /bin/false
install cramfs /bin/false
install freevxfs /bin/false
install jffs2 /bin/false
install hfs /bin/false
install hfsplus /bin/false
install firewire-core /bin/false
install firewire-ohci /bin/false
EOF
        chmod 0644 "$BLACKLIST_FILE"
        echo "[aenes-security] Blacklist de módulos legados aplicada."
    else
        rm -f "$BLACKLIST_FILE"
        echo "[aenes-security] Blacklist de módulos legados removida."
    fi
}

apply_hosts_adblock() {
    local backup="/etc/hosts.aenes-backup"
    local tmp
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' RETURN
    [ -f "$backup" ] || cp -a /etc/hosts "$backup"
    echo "[aenes-security] Baixando lista anti-malware e trackers (StevenBlack unified)..."
    curl -fsSL "https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts" -o "$tmp"
    {
        cat "$backup"
        printf '\n# aenes-hosts-adblock START\n'
        grep -E '^0\.0\.0\.0 ' "$tmp"
        printf '# aenes-hosts-adblock END\n'
    } > /etc/hosts.aenes-new
    mv /etc/hosts.aenes-new /etc/hosts
    chmod 0644 /etc/hosts
    echo "[aenes-security] /etc/hosts atualizado ($(grep -c '^0\.0\.0\.0 ' /etc/hosts) domínios bloqueados). Backup em $backup."
}

undo_hosts_adblock() {
    local backup="/etc/hosts.aenes-backup"
    if [ -f "$backup" ]; then
        cp -a "$backup" /etc/hosts
        chmod 0644 /etc/hosts
        echo "[aenes-security] /etc/hosts restaurado do backup original."
    else
        sed -i '/# aenes-hosts-adblock START/,/# aenes-hosts-adblock END/d' /etc/hosts
        echo "[aenes-security] Bloco anti-tracker removido de /etc/hosts."
    fi
}

usbguard_setup() {
    if ! command -v usbguard >/dev/null 2>&1; then
        echo "[aenes-security] Instalando usbguard via pacman..."
        pacman -S --needed --noconfirm usbguard
    fi
    echo "[aenes-security] Gerando política permitindo TODOS os dispositivos USB conectados agora..."
    usbguard generate-policy > /etc/usbguard/rules.conf
    chmod 0600 /etc/usbguard/rules.conf
    systemctl enable --now usbguard.service
    echo "[aenes-security] USBGuard ativo! Novos USBs desconhecidos serão bloqueados até autorização (usbguard list-devices / allow-device)."
}

usbguard_disable() {
    systemctl disable --now usbguard.service 2>/dev/null || true
    echo "[aenes-security] USBGuard desativado."
}

boot_armor_info() {
    echo "=== Parâmetros de Kernel Atuais ==="
    cat /proc/cmdline 2>/dev/null || true
    echo ""
    echo "=== Kernel Lockdown ==="
    cat /sys/kernel/security/lockdown 2>/dev/null || true
    echo ""
    echo "=== Flags recomendadas para blindagem de memória no boot ==="
    echo "  lockdown=integrity init_on_alloc=1 slab_nomerge page_alloc.shuffle=1"
    echo ""
    if [ -d /boot/loader/entries ]; then
        echo "Bootloader detectado: systemd-boot (/boot/loader/entries/)"
        ls -1 /boot/loader/entries/
    elif [ -f /boot/limine.conf ] || [ -f /boot/limine/limine.conf ]; then
        echo "Bootloader detectado: Limine"
    elif [ -f /etc/default/grub ]; then
        echo "Bootloader detectado: GRUB (/etc/default/grub -> GRUB_CMDLINE_LINUX_DEFAULT)"
    fi
}

case "${1:-}" in
    --kernel) apply_kernel ;;
    --kernel-undo) undo_kernel ;;
    --kernel-armor) apply_kernel_armor ;;
    --kernel-armor-undo) undo_kernel_armor ;;
    --toggle-icmp-stealth) toggle_sysctl_dropin icmp-stealth net.ipv4.icmp_echo_ignore_all 1 0 "${2:-on}" ;;
    --toggle-ipv6-privacy) toggle_sysctl_dropin ipv6-privacy net.ipv6.conf.all.use_tempaddr 2 0 "${2:-on}" ;;
    --toggle-io-uring) toggle_sysctl_dropin io-uring kernel.io_uring_disabled 1 0 "${2:-on}" ;;
    --toggle-sysrq) toggle_sysctl_dropin sysrq kernel.sysrq 0 1 "${2:-on}" ;;
    --toggle-nocoredump) toggle_nocoredump "${2:-on}" ;;
    --toggle-blacklist) toggle_blacklist "${2:-on}" ;;
    --hosts-adblock) apply_hosts_adblock ;;
    --hosts-undo) undo_hosts_adblock ;;
    --usbguard-setup) usbguard_setup ;;
    --usbguard-disable) usbguard_disable ;;
    --boot-armor-info) boot_armor_info ;;
    --nm-privacy) apply_nm_privacy ;;
    --nm-privacy-undo) undo_nm_privacy ;;
    --dns) apply_dns "${2:-quad9}" ;;
    --firewall-strict) apply_firewall_strict ;;
    *)
        echo "Uso: sudo $0 --kernel | --kernel-armor | --toggle-* on|off | --hosts-adblock | --hosts-undo | --usbguard-setup | --usbguard-disable | --boot-armor-info | --nm-privacy | --dns <provider> | --firewall-strict"
        exit 1
        ;;
esac
