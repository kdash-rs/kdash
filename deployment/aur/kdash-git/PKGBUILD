# Maintainer: Deepu K Sasidharan <d4udts@gmail.com>
#
# Static PKGBUILD -- pkgver() is computed from git describe at build time,
# so there is no template substitution and no per-release rendering. The
# publish-aur-git release job only refreshes .SRCINFO and pushes when this
# file itself changes.
pkgname=kdash-git
_pkgname=kdash
pkgver=0.0.1.r0.g0000000
pkgrel=1
pkgdesc='A fast and simple dashboard for Kubernetes (git main)'
arch=('x86_64' 'aarch64')
url='https://github.com/kdash-rs/kdash'
license=('MIT')
depends=('gcc-libs')
makedepends=('cargo' 'git')
optdepends=('libxcb: copy-to-clipboard support')
provides=('kdash')
conflicts=('kdash' 'kdash-bin')
options=('!lto')
source=("$_pkgname::git+$url.git#branch=main")
sha256sums=('SKIP')

pkgver() {
  cd "$_pkgname"
  # Tag form vX.Y.Z[-suffix] -> X.Y.Z[.suffix].rN.g<sha>. Falls back to
  # rN.g<sha> for repos that haven't been tagged yet.
  ( git describe --long --tags --abbrev=7 2>/dev/null \
      | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g' ) \
    || printf 'r%s.%s' "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
}

build() {
  cd "$_pkgname"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --release --locked
}

package() {
  cd "$_pkgname"
  install -Dm755 target/release/kdash "$pkgdir/usr/bin/kdash"
  install -Dm644 LICENSE      "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md    "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 CHANGELOG.md "$pkgdir/usr/share/doc/$pkgname/CHANGELOG.md"
}
