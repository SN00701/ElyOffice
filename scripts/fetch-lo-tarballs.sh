#!/usr/bin/env bash
# Fetches the external tarballs the LibreOffice build needs, several at a
# time.  One connection on a slow line is the bottleneck, not the total
# size: the build's own fetch does them one after another, which turns a
# few megabytes into an hour.
#
# Usage: fetch-lo-tarballs.sh <lo-src> [jobs]
#
# The build still fetches whatever is missing; this only gets there first.
set -euo pipefail

# Cygwin's own tools; the Windows PATH is not imported (it carries spaces).
export PATH=/usr/local/bin:/usr/bin:/bin

src=${1:?usage: fetch-lo-tarballs.sh <lo-src> [jobs]}
jobs=${2:-6}
tarballs="$src/external/tarballs"
list="$src/download.lst"
base=${ELY_LO_TARBALL_BASE:-https://dev-www.libreoffice.org/src}

if [ ! -f "$list" ]; then
  echo "fetch-lo-tarballs.sh: $list is not there" >&2
  exit 1
fi
mkdir -p "$tarballs"

# The packages the build asks for: the ones in its build type.
build_type=" $(sed -n 's/^export BUILD_TYPE=//p' "$src/config_host.mk") "
wanted=$(sed -n 's/.*fetch_Optional,\([A-Z_0-9]*\),\([A-Z_0-9]*\)_TARBALL.*/\1/p' "$src/Makefile.fetch" | sort -u)
# The packages the build asks for whatever the build type: they are named
# without a build type.
always=$(sed -n 's/.*[^_]\([A-Z_0-9]*\)_TARBALL.*/\1/p' "$src/Makefile.fetch" | sort -u)

# The name and the checksum of each package.
name_of() { sed -n "s/^$1_TARBALL := //p" "$list" | head -1; }
sum_of() { sed -n "s/^$1_SHA256SUM := //p" "$list" | head -1; }
# Some names carry a version the build fills in; it is in the build's own
# settings, so it is taken from there.
expand() {
  local name=$1
  while [[ $name == *'$('* ]]; do
    local variable=${name#*'$('}
    variable=${variable%%'${'*}
    variable=${variable%%')'*}
    local value
    value=$(sed -n "s/^export $variable=//p" "$src/config_host.mk" | head -1)
    if [ -z "$value" ]; then
      return 1
    fi
    name=${name//"\$($variable)"/$value}
  done
  echo "$name"
}

queue=()
for package in $wanted $always; do
  case " $build_type " in
    *" $package "*) ;;
    *) continue ;;
  esac
  name=$(name_of "$package")
  sum=$(sum_of "$package")
  [ -n "$name" ] || continue
  [ -n "$sum" ] || continue
  if ! name=$(expand "$name"); then
    echo "fetch-lo-tarballs.sh: the name of $package is not known here; the build will fetch it"
    continue
  fi
  if [ -f "$tarballs/$name" ]; then
    if [ "$(sha256sum "$tarballs/$name" | cut -d' ' -f1)" = "$sum" ]; then
      continue
    fi
    echo "fetch-lo-tarballs.sh: $name is not what it should be; fetching it again"
    rm -f "$tarballs/$name"
  fi
  queue+=("$name|$sum")
done

if [ ${#queue[@]} -eq 0 ]; then
  echo "fetch-lo-tarballs.sh: every tarball is in place"
  exit 0
fi

echo "fetch-lo-tarballs.sh: ${#queue[@]} tarballs to fetch, $jobs at a time"
download() {
  local name=${1%%|*} sum=${1##*|}
  local out="$tarballs/$name.part"
  rm -f "$out"
  # A connection that stops delivering is given up on and tried again.
  for attempt in 1 2 3; do
    if timeout 1800 wget --progress=dot:mega -q -c -O "$out" "$base/$name"; then
      if [ "$(sha256sum "$out" | cut -d' ' -f1)" = "$sum" ]; then
        mv "$out" "$tarballs/$name"
        echo "fetch-lo-tarballs.sh: $name is in place"
        return 0
      fi
      echo "fetch-lo-tarballs.sh: $name has the wrong checksum" >&2
    fi
    rm -f "$out"
    sleep 5
  done
  echo "fetch-lo-tarballs.sh: $name could not be fetched" >&2
  return 1
}
export -f download
export tarballs base

printf '%s\n' "${queue[@]}" | xargs -P "$jobs" -I{} bash -c 'download "$@"' _ {}
