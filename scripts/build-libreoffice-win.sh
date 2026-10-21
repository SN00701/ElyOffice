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

# Windows reads a name the same whatever its case, but the environment of the
# build tools is read with the case, and two spellings of one name make them
# fall over ("an item with the same key has already been added").  The name
# in capitals is the one the build tools use, so the one in small letters is
# let go.
unset no_proxy http_proxy https_proxy all_proxy ftp_proxy
export NO_PROXY=${NO_PROXY:-127.0.0.1,localhost}
export HTTP_PROXY=${HTTP_PROXY:-http://127.0.0.1:7897}
export HTTPS_PROXY=${HTTPS_PROXY:-http://127.0.0.1:7897}

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

# One request for every file the patches need, and the whole shape of the
# tree: the build system reads files from every part of it.  The sources
# themselves are fetched as the build asks for them.
if [ -n "$patched" ]; then
  rootdirs=`git -C "$src" ls-tree --name-only HEAD | grep -v '\.md$' | tr '\n' ' '`
  git -C "$src" sparse-checkout set --no-cone '/*' '!/*/' $rootdirs $patched
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

# The tiled rendering on Windows draws through the Win32 device, and the
# patch 0044 is about that device.  Skia and the PDF import of LibreOffice
# are not needed for it, and they would ask for a clang-cl that this
# machine does not have.
if [ "${ELY_WITH_SKIA:-no}" = "yes" ]; then
  options+=(--enable-skia --enable-pdfium --enable-pdfimport)
else
  # The PDF import of LibreOffice (opening a PDF in Draw) is a thing of its
  # own and asks for the same kernel; the window has a PDF engine of its
  # own, and the export of a document to PDF does not need either.
  options+=(--disable-skia --disable-pdfium --disable-pdfimport)
fi

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

# The build wants clang-cl for the parts that Skia and PDFium bring with
# them, and Visual Studio does not always have it.  Cygwin's own does.
if [ -z "${ELY_CLANG_CL:-}" ] && [ -x /cygdrive/f/cygwin64/bin/clang-cl.exe ]; then
  export ELY_CLANG_CL=/cygdrive/f/cygwin64/bin/clang-cl.exe
  echo "build-libreoffice-win: clang-cl comes from Cygwin"
fi

# The compiler needs the environment Visual Studio sets up: the headers
# and the libraries of the tool set and of the Windows SDK.  A batch file
# of our own asks Visual Studio for it, because a command line with the
# path of the installer in it is full of quotes that never survive.
if [ -z "${MSVC_ENV_LOADED:-}" ]; then
  vs_tools=/cygdrive/f/VS18/VC/Tools/MSVC
  for toolset in `ls $vs_tools 2>/dev/null`; do
    if [ -f "$vs_tools/$toolset/bin/HostX64/x64/cl.exe" ]; then
      vs_toolset=$toolset
      break
    fi
  done
  batch=$ELY_LO_ROOT/vsenv.bat
  cat > "$batch" <<BAT
@call "D:\\Program Files\\Microsoft Visual Studio\\18\\Community\\Common7\\Tools\\VsDevCmd.bat" -arch=x64 -host_arch=x64 -vcvars_ver=$vs_toolset -no_logo
@set
BAT
  envfile=$ELY_LO_ROOT/msvc-env.txt
  cmd.exe /c "`cygpath -w "$batch"`" > "$envfile" 2>/dev/null || true
  if [ -s "$envfile" ]; then
    # The names that are in the environment already.  A name that differs
    # only in its case is the same name to Windows and to the build tools,
    # and two of them make them fall over ("an item with the same key has
    # already been added").
    imported=$(env | sed -n 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/\1/p' | tr 'A-Z' 'a-z' | tr '\n' ' ')
    while IFS= read -r line; do
      key=${line%%=*}
      value=${line#*=}
      value=${value%$'\r'}
      case "$key" in
        [A-Za-z_]*) ;;
        *) continue ;;
      esac
      case "$key" in
        *[!A-Za-z0-9_]*) continue ;;
      esac
      case "$key" in
        PATH | Path) continue ;;
      esac
      # The include list of the compiler is a list of paths separated by
      # semicolons, and the build puts it into the commands it writes.  The
      # paths of Visual Studio and of the Windows SDK have spaces and
      # parentheses in them, so the shell that runs those commands falls over.
      # The build has its own include flags (SOLARINC), and they name the
      # same folders, so the list is not needed here.
      case "$key" in
        INCLUDE) continue ;;
      esac
      # The build tools put their own name for the version of the C library
      # into the environment of the compiler, and they fall over when the
      # environment has that name already under another spelling.
      case "$key" in
        UCRTVersion | UCRTVERSION) continue ;;
      esac
      # A name that differs only in its case is the same name to Windows and
      # to the build tools, and two of them in the environment make them
      # fall over ("an item with the same key has already been added").
      lower=$(printf '%s' "$key" | tr 'A-Z' 'a-z')
      case " $imported " in
        *" $lower "*) continue ;;
      esac
      imported="$imported $lower"
      export "$key=$value"
    done < "$envfile"
    tools=${VCToolsInstallDir%/}
    PATH="$PATH:$(cygpath -u "$tools")/bin/HostX64/x64"
    PATH="$PATH:$(cygpath -u "$WindowsSdkVerBinPath")"
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
  powershell.exe -NoProfile -Command "if (-not (Test-Path 'F:\VS18\Common7')) { cmd /c mklink /J F:\VS18 \"D:\Program Files\Microsoft Visual Studio\18\Community\" }" >/dev/null 2>&1 || true
  if [ -d "/cygdrive/f/VS18/VC/Tools/MSVC" ]; then
    export ELY_VS_ALIAS='F:\VS18'
    echo "build-libreoffice-win: Visual Studio is reached through $ELY_VS_ALIAS"
  fi
fi

# Cygwin's make, never Git Bash's; MSYSTEM misleads the build.
unset MSYSTEM MSYSTEM_PREFIX MINGW_PREFIX MSYSTEM_CHOST MSYSTEM_CARCH
export PATH=/opt/lo/bin:$PATH
echo "build-libreoffice-win: the path is $PATH"

# The build asks the packages it builds itself for their flags, and a
# machine without them has no answer.  The stand-in answers for the ones the
# build has of its own; the folders and the libraries themselves are given
# to the compiler by the build.
if [ ! -x /opt/lo/bin/pkg-config ] && [ -f "$root/scripts/pkgconf-stub.rs" ]; then
  if command -v rustc >/dev/null 2>&1; then
    mkdir -p /opt/lo/bin
    (cd "$root" && rustc -O -o "$(cygpath -w /opt/lo/bin)/pkgconf-2.4.3.exe" scripts/pkgconf-stub.rs) || true
    cp /opt/lo/bin/pkgconf-2.4.3.exe /opt/lo/bin/pkg-config 2>/dev/null || true
    echo "build-libreoffice-win: the stand-in for pkg-config is made"
  fi
fi
# The packages ask for their flags by name; the stand-in answers for the ones
# the build has of its own.  The name in the build's own settings is empty,
# and an empty name keeps the packages from looking for one at all.
export PKG_CONFIG=/opt/lo/bin/pkg-config
if [ -f config_host.mk ]; then
  sed -i "s|^export PKG_CONFIG=.*|export PKG_CONFIG=$(cygpath -m /opt/lo/bin/pkg-config)|" config_host.mk
fi

# The externals that are built with MSBuild ask for the targets of the Visual
# Studio that is installed, by the name of its version.  This installation has
# the C++ targets of Visual Studio 2022 (the folder v170) and not those of
# the newer one (v180), and the folder of the newer one cannot be written to
# (it belongs to the installer).  The targets are told where to look, and the
# toolset is the one that has the compiler, which the older targets know.
if [ -n "${ELY_VS_ALIAS:-}" ]; then
  targets="$ELY_VS_ALIAS/MSBuild/Microsoft/VC"
  if [ ! -f "$targets/v180/Microsoft.Cpp.Default.props" ] && [ -f "$targets/v170/Microsoft.Cpp.Default.props" ]; then
    export VCTargetsPath="$targets/v170\\"
    echo "build-libreoffice-win: the C++ targets of Visual Studio 2022 are used"
  fi
fi

cd "$src"
if [ ! -f config_host.mk ] || ! printf '%s\n' "${options[@]}" | cmp -s - autogen.lastrun; then
  ./autogen.sh "${options[@]}"
fi

# The fonts are not shipped, so the kernel neither fetches nor installs them
# (ADR-003).  MORE_FONTS is what brings the Noto, DejaVu and Liberation
# families into the build; without it, the download of those tarballs, which
# takes hours on a slow line, is not started at all.
if [ -f config_host.mk ]; then
  sed -i 's/ MORE_FONTS / /' config_host.mk
  echo "build-libreoffice-win: the fonts are left out of the build"
fi

# Visual Studio may come without a CMake of its own, and the build writes
# the path of the one it found into the commands it runs, so the path may not
# have a space in it.  The CMake that is on the machine stands in, reached
# through a junction (a junction needs no administrator rights).
if [ -f config_host.mk ] && ! [ -f /cygdrive/f/VS18/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe ]; then
  powershell.exe -NoProfile -Command "if (-not (Test-Path 'F:\CMake\bin\cmake.exe')) { cmd /c mklink /J F:\CMake \"D:\Program Files\CMake\" }" >/dev/null 2>&1 || true
  if [ -f /cygdrive/f/CMake/bin/cmake.exe ]; then
    sed -i 's|^CMAKE=.*|CMAKE=F:/CMake/bin/cmake.exe|' config_host.mk
    echo "build-libreoffice-win: the CMake of the machine is used"
  fi
fi

# The build fetches its tarballs one after another, and one connection on a
# slow line is the bottleneck.  They are fetched here, several at a time, so
# the build finds them in place.
if [ -f download.lst ]; then
  bash "$root/scripts/fetch-lo-tarballs.sh" "$src" "${ELY_LO_FETCH_JOBS:-6}" || true
fi

make -j"$jobs" VCTOOLSET=v143

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
