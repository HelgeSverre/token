#!/bin/sh
# Bundle Token and add the Freedesktop registration files that cargo-bundle
# cannot place in their system discovery directories itself.
set -eu

target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$target" in
    *-unknown-linux-gnu) ;;
    *)
        echo "unsupported Linux target: $target" >&2
        exit 2
        ;;
esac

cargo bundle --release --target "$target" --format deb --bin token

package="$(printf '%s\n' "target/$target/release/bundle/deb/"*.deb | head -n 1)"
if [ ! -f "$package" ]; then
    echo "cargo-bundle did not produce a Debian package" >&2
    exit 1
fi

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT HUP INT TERM
root="$work_dir/root"

xmllint --noout assets/linux/no.helgesverre.token.xml
desktop-file-validate assets/linux/no.helgesverre.token.desktop
dpkg-deb --extract "$package" "$root"
rm -f "$root/usr/share/applications/"*.desktop
install -D -m 644 assets/linux/no.helgesverre.token.desktop \
    "$root/usr/share/applications/no.helgesverre.token.desktop"
install -D -m 644 assets/linux/no.helgesverre.token.xml \
    "$root/usr/share/mime/packages/no.helgesverre.token.xml"
dpkg-deb --build "$root" "$package"
dpkg-deb --contents "$package" | grep -Fq './usr/share/applications/no.helgesverre.token.desktop'
dpkg-deb --contents "$package" | grep -Fq './usr/share/mime/packages/no.helgesverre.token.xml'
echo "$package"
