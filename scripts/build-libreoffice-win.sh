#!/usr/bin/env bash
# Builds the pinned LibreOffice with Ely's LOK patches on Windows (Cygwin).
#
# Usage: scripts/build-libreoffice-win.sh
# Run it from Cygwin bash, with the JDK on PATH:
#   PATH="/cygdrive/c/Program Files/Microsoft/jdk-17.0.19.10-hotspot/bin:$PATH" \
#     scripts/build-libreoffice-win.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
# The source lives outside the repository, and outside any path with a space:
# LibreOffice's autogen.sh refuses a source path that has one.
if [ -z "${ELY_LO_ROOT:-}" ]; then
  export ELY_LO_ROOT=/cygdrive/f/mpely-lo
fi
src=$ELY_LO_ROOT/src
pin=8221e31b3ac356a1623c672912a3d2b492f7e3d1
jobs=${ELY_BUILD_JOBS:-$(nproc)}

mkdir -p "$ELY_LO_ROOT"

# The files Ely's patches touch.  The tree is a partial clone, so those blobs
# are not here yet; checking them out fetches them in one request instead of
# one request per file, which is the difference between minutes and hours.
patched=$(cd "$root" && sed -n 's|^+++ b/||p' patches/libreoffice/*.patch | sort -u)
patched_dirs=$(printf '%s\n' "$patched" | xargs -n1 dirname | sort -u)

# The sources live outside the repository so the tree stays small.
if [ ! -d "$src/.git" ]; then
  mkdir -p "$src"
  git -C "$src" init -q
  git -C "$src" config core.autocrlf false
  git -C "$src" config core.eol lf
  git -C "$src" remote add origin https://github.com/LibreOffice/core.git
  git -C "$src" sparse-checkout init --no-cone
  git -C "$src" sparse-checkout set \
    include/LibreOfficeKit libreofficekit desktop/source/lib desktop/inc \
    officecfg/registry/data/org/openoffice/Office/UI
  git -C "$src" fetch -q --depth 1 --filter=blob:none origin "$pin"
  git -C "$src" checkout -q FETCH_HEAD
fi
[ "$(git -C "$src" rev-parse HEAD)" = "$pin" ] || {
  echo "build-libreoffice-win: $src is not at $pin" >&2
  exit 1
}

# One request for every file the patches need.  The root's own files come
# along too: autogen.sh, configure.ac, Makefile.in and the macros in m4/ are
# what the build starts from.
if [ -n "$patched" ]; then
  git -C "$src" sparse-checkout set --no-cone \
    '/*' '!/*/' m4/ \
    include/LibreOfficeKit libreofficekit desktop/source/lib desktop/inc \
    officecfg/registry/data/org/openoffice/Office/UI $patched
  echo "build-libreoffice-win: fetching the patched files"
  git -C "$src" read-tree -mu HEAD
fi

if [ ! -f "$src/autogen.sh" ]; then
  echo "build-libreoffice-win: the tree has no autogen.sh" >&2
  exit 1
fi

"$root/scripts/apply-lo-patches.sh" "$src"

options=(
  --enable-bogus-pkg-config
  --enable-mergelibs
  --enable-release-build
  --without-java
  --without-junit
  --without-help
  --disable-odk
  --disable-online-update
  --disable-extension-integration
  --disable-scripting-beanshell
  --disable-scripting-javascript
  --disable-firebird-sdbc
  --disable-postgresql-sdbc
  --disable-ext-nlpsolver
  --disable-ext-wiki-publisher
  --disable-report-builder
  --disable-sdremote
  --disable-sdremote-bluetooth
  --disable-poppler
  --disable-lotuswordpro
  --disable-gpgmepp
  --disable-cups
  --disable-evolution2
  --with-galleries=no
  --with-lang=
  --with-theme=colibre
  --with-fonts
  --disable-python
  --disable-coinmp
  --disable-lpsolve
  --without-doxygen
  --with-myspell-dicts
)

# Unix-only switches do not apply here.
kept=()
for option in "${options[@]}"; do
  case $option in
    --enable-bogus-pkg-config | --disable-cups | --disable-evolution2 | --disable-python) ;;
    *) kept+=("$option") ;;
  esac
done
options=("${kept[@]}" --disable-ccache --disable-cli --without-dotnet)

# The Visual Studio that is here.  LibreOffice knows 2022 and 2026; this
# machine has the 2026 one (version 18, tool set v145).
vs=${ELY_VS_VERSION:-2022}
if [ -d "/cygdrive/d/Program Files/Microsoft Visual Studio/18/Community" ]; then
  vs=${ELY_VS_VERSION:-2026}
fi
options+=(--with-visual-studio="$vs")

# The compiler needs the environment Visual Studio sets up: the headers
# and the libraries of the tool set and of the Windows SDK.  Take what
# Visual Studio's own script leaves behind, but take the tool set that
# has the compiler -- a newer one may hold only its auxiliary files.
if [ -z "${MSVC_ENV_LOADED:-}" ]; then
  vs_tools=/cygdrive/f/VS18/VC/Tools/MSVC
  for toolset in `ls $vs_tools 2>/dev/null`; do
    if [ -f "$vs_tools/$toolset/bin/HostX64/x64/cl.exe" ]; then
      vs_toolset=$toolset
      break
    fi
  done
  envfile=$ELY_LO_ROOT/msvc-env.txt
  MSYS2_ARG_CONV_EXCL='*' cmd.exe /c "call \"D:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat\" -arch=x64 -host_arch=x64 -vcvars_ver=$vs_toolset -no_logo && set" > "$envfile" 2>/dev/null || true
  if [ -s "$envfile" ]; then
    while IFS= read -r line; do
      value=${line#*=}
      value=${value%$'\r'}
      case "$line" in
        PATH=* | Path=*) continue ;;
        [A-Za-z_]*=*)
          key=${line%%=*}
          export "$key=$value"
          ;;
      esac
    done < "$envfile"
    # The PATH from Visual Studio is in the Windows form; the shell wants
    # its own.
    for dir in "$VCToolsInstallDir" "$WindowsSdkVerBinPath"; do
      case "$dir" in
        [A-Z]:*)
          unix=`cygpath -u "$dir" 2>/dev/null` && PATH="$PATH:$unix"
          ;;
      esac
    done
    export PATH
    export MSVC_ENV_LOADED=1
    echo "build-libreoffice-win: the compiler environment is set (tool set $vs_toolset)"
  fi
fi

# Visual Studio sits in a folder whose name has a space, and LibreOffice's
# configure cannot use such a path (it would need the short names, which this
# machine has turned off).  A junction with a short name of its own solves
# it, and a junction needs no administrator rights.
if [ -z "${ELY_VS_ALIAS:-}" ] && [ "$vs" = "2026" ]; then
  powershell.exe -NoProfile -Command "if (-not (Test-Path 'F:\VS18\Common7')) { cmd /c mklink /J F:\VS18 'D:\Program Files\Microsoft Visual Studio\18\Community' }" >/dev/null 2>&1 || true
  if [ -d "/cygdrive/f/VS18/VC/Tools/MSVC" ]; then
    export ELY_VS_ALIAS='F:\VS18'
    echo "build-libreoffice-win: Visual Studio is reached through $ELY_VS_ALIAS"
  fi
fi

# Cygwin's make, never Git Bash's; MSYSTEM misleads the build.
unset MSYSTEM MSYSTEM_PREFIX MINGW_PREFIX MSYSTEM_CHOST MSYSTEM_CARCH
export PATH=/opt/lo/bin:$PATH

cd "$src"
if [ ! -f config_host.mk ] || ! printf '%s\n' "${options[@]}" | cmp -s - autogen.lastrun; then
  ./autogen.sh "${options[@]}"
fi
make -j"$jobs"

# Only the English dictionary; each one slows the first start.
app=$src/instdir/share
keep=(dict-en)
extensions=$app/extensions
for dict in "$extensions"/dict-*; do
  name=$(basename "$dict")
  case " ${keep[*]} " in
    *" $name "*) ;;
    *) rm -rf "$dict" ;;
  esac
done

# Lightproof is Python: inert without it, a hang on Windows.
en=$extensions/dict-en
if [ -d "$en" ]; then
  rm -f "$en/Lightproof.py" "$en/Lightproof.components" "$en/Linguistic.xcu"
  rm -rf "$en/pythonpath" "$en/dialog"
fi

# Ely's kernel defaults.
cp "$root/packaging/libreoffice/ely.xcd" "$app/registry/"

echo "build-libreoffice-win: done"
echo "  kernel: $app"
ls -1 "$app" | head -20
