# Maintainer: Alexandre <alexandre.enes1@gmail.com>
#
# cosmic-cosmetics: daemon, holder, logi, instalador e greeter (x86-64-v3).
# NÃO conflita com cosmic-panel/cosmic-notifications: os originais continuam
# instalados (fallback). Nada é habilitado aqui; /etc e /usr/local são do
# instalador (reversível), por usuário.
#
#   Construir (não instala):  cd packaging/arch/cosmic-cosmetics && makepkg -f
#   Instalar:                 sudo pacman -U cosmic-cosmetics-0.1.0-1-x86_64.pkg.tar.zst
#   Ativar (como usuário):    cosmic-cosmetics-install install --dry-run ; cosmic-cosmetics-install install
#   Voltar:                   cosmic-cosmetics-install uninstall ; sudo pacman -R cosmic-cosmetics
#
# Compilado para x86-64-v3, sem target-cpu=native.

pkgname=cosmic-cosmetics
pkgver=0.1.0
pkgrel=1
pkgdesc='COSMIC cosmetics: daemon, modules, installer and greeter (x86-64-v3)'
arch=(x86_64)
url='https://github.com/pop-os/cosmic-epoch'
license=(GPL-3.0-only)
depends=(cosmic-session cosmic-comp greetd pam systemd-libs libxkbcommon fontconfig wayland udev polkit)
makedepends=(cargo)
install=cosmic-cosmetics.install
options=(!lto !debug)

# startdir = packaging/arch/cosmic-cosmetics => raiz do repositório três níveis acima.
_repo="$(realpath "$startdir/../../..")"
# Target separado: o RUSTFLAGS do makepkg invalidaria o cache de dev.
_target="$_repo/target/aenes-makepkg-cosmetics"

_v3_flags() {
  export RUSTFLAGS="-C target-cpu=x86-64-v3"
  CFLAGS="${CFLAGS//-march=native/-march=x86-64-v3}"; CFLAGS="${CFLAGS//-mtune=native/-mtune=generic}"
  CXXFLAGS="${CXXFLAGS//-march=native/-march=x86-64-v3}"; CXXFLAGS="${CXXFLAGS//-mtune=native/-mtune=generic}"
  export CFLAGS CXXFLAGS
  case "$RUSTFLAGS $CFLAGS $CXXFLAGS" in *native*) echo "flags com native: $RUSTFLAGS | $CFLAGS" >&2; return 1 ;; esac
}

prepare() {
  cd "$_repo"
  export CARGO_TARGET_DIR="$_target"
  cargo fetch --locked
}

build() {
  _v3_flags
  cd "$_repo"
  export CARGO_TARGET_DIR="$_target" CARGO_PROFILE_RELEASE_LTO=thin
  cargo build --release --frozen \
    -p cosmic-cosmeticsd -p cosmic-cosmetics-holder -p cosmic-cosmetics-logi \
    -p cosmic-cosmetics-installer -p cosmic-cosmetics-greeter -p cosmic-cosmetics-greeter-daemon
}

package() {
  cd "$_repo"
  local bin
  for bin in cosmic-cosmeticsd cosmic-cosmetics-holder cosmic-cosmetics-logi \
             cosmic-cosmetics-install cosmic-cosmetics-greeter cosmic-cosmetics-greeter-daemon; do
    install -Dm755 "$_target/release/$bin" "$pkgdir/usr/bin/$bin"
  done
  install -Dm755 "$_target/release/cosmic-cosmetics-shim" \
    "$pkgdir/usr/lib/cosmic-cosmetics/cosmic-cosmetics-shim"

  # Units de usuário (não habilitadas: o habilitar é do instalador, por usuário).
  local unit
  for unit in "$_repo"/packaging/systemd/*.service; do
    sed 's#%h/.local/bin/#/usr/bin/#g' "$unit" \
      > "$srcdir/$(basename "$unit")"
    install -Dm644 "$srcdir/$(basename "$unit")" \
      "$pkgdir/usr/lib/systemd/user/$(basename "$unit")"
  done

  # Política D-Bus com.system76.CosmicGreeter.conf NÃO é instalada: o mesmo
  # caminho pertence ao cosmic-greeter(-aenes) (conteúdo idêntico, conflito de
  # arquivo no pacman) e já cobre o BusName com.system76.CosmicGreeter.
}
