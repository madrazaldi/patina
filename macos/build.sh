#!/usr/bin/env bash
# Builds Patina.app: the Rust backend as a static library (crates/ffi), then
# the SwiftUI app linked against it, bundled and signed.
#
#   macos/build.sh                 debug build of this Mac's architecture
#   macos/build.sh --release       optimized
#   macos/build.sh --release --universal --dmg
#
# Signs with $APPLE_SIGNING_IDENTITY (hardened runtime) or ad hoc when unset.
# Output: macos/build/Patina.app (and Patina.dmg with --dmg).
set -euo pipefail

cd "$(dirname "$0")/.."
root=$(pwd)
profile=debug
universal=false
dmg=false
for arg in "$@"; do
  case $arg in
    --release) profile=release ;;
    --universal) universal=true ;;
    --dmg) dmg=true ;;
    *) echo "unknown option $arg" >&2; exit 2 ;;
  esac
done

# SwiftUI's macros ship with Xcode, not the Command Line Tools.
if [ -z "${DEVELOPER_DIR:-}" ] && [[ "$(xcode-select -p)" == */CommandLineTools ]]; then
  xcode=$(mdfind "kMDItemCFBundleIdentifier == com.apple.dt.Xcode" | head -1)
  [ -n "$xcode" ] || { echo "Xcode is required to build the SwiftUI app." >&2; exit 1; }
  export DEVELOPER_DIR="$xcode/Contents/Developer"
fi

export MACOSX_DEPLOYMENT_TARGET=14.0
version=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
cargo_flags=(-p patina-ffi --locked)
[ $profile = release ] && cargo_flags+=(--release)
cargo_dir=$([ $profile = release ] && echo release || echo debug)

if $universal; then
  targets=(aarch64-apple-darwin x86_64-apple-darwin)
  for t in "${targets[@]}"; do cargo build "${cargo_flags[@]}" --target "$t"; done
  lib_dir="$root/target/universal/$cargo_dir"
  mkdir -p "$lib_dir"
  libs=()
  for t in "${targets[@]}"; do libs+=("$root/target/$t/$cargo_dir/libpatina_ffi.a"); done
  lipo -create "${libs[@]}" -output "$lib_dir/libpatina_ffi.a"
  swift_arch=(--arch arm64 --arch x86_64)
else
  cargo build "${cargo_flags[@]}"
  lib_dir="$root/target/$cargo_dir"
  swift_arch=()
fi

swift build --package-path macos -c $profile ${swift_arch[@]+"${swift_arch[@]}"} -Xlinker -L"$lib_dir"
bin=$(swift build --package-path macos -c $profile ${swift_arch[@]+"${swift_arch[@]}"} --show-bin-path)/Patina

app=macos/build/Patina.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/Patina"
cp macos/Resources/AppIcon.icns "$app/Contents/Resources/"
sed "s/\$(VERSION)/$version/g" macos/Info.plist > "$app/Contents/Info.plist"

identity=${APPLE_SIGNING_IDENTITY:--}
if [ "$identity" = "-" ]; then
  codesign --force --sign - "$app"
else
  codesign --force --options runtime --timestamp --sign "$identity" "$app"
fi
echo "Built $app ($version, $profile)"

if $dmg; then
  out=macos/build/Patina.dmg
  staging=$(mktemp -d)
  cp -R "$app" "$staging/"
  ln -s /Applications "$staging/Applications"
  rm -f "$out"
  hdiutil create -volname Patina -srcfolder "$staging" -fs HFS+ -format UDZO "$out" >/dev/null
  rm -rf "$staging"
  [ "$identity" = "-" ] || codesign --force --sign "$identity" "$out"
  echo "Built $out"
fi
