#!/usr/bin/env bash
# Runs the Windows LibreOffice build from Cygwin with a clean environment.
#
# The Windows PATH is not imported: its entries carry spaces, which breaks
# Cygwin's own tools. The JDK and the system directories are added by hand.
set -euo pipefail

export PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
export PATH="$PATH:/cygdrive/c/Windows/system32:/cygdrive/c/Windows"
export PATH="$PATH:/cygdrive/c/Program Files/Microsoft/jdk-17.0.19.10-hotspot/bin"

export HTTP_PROXY=${HTTP_PROXY:-http://127.0.0.1:7897}
export HTTPS_PROXY=${HTTPS_PROXY:-http://127.0.0.1:7897}
export no_proxy=127.0.0.1,localhost

export ELY_LO_ROOT=${ELY_LO_ROOT:-/cygdrive/f/mpely-lo}
export ELY_BUILD_JOBS=${ELY_BUILD_JOBS:-16}

repo=${ELY_REPO:-/cygdrive/e/trae pg/office_modpack/mpelyoffice}
log=${ELY_LO_LOG:-/cygdrive/f/mpely-lo/build.log}
mkdir -p "$(dirname "$log")"
cd "$repo"
set -o pipefail
bash scripts/build-libreoffice-win.sh 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
echo "exit=$status" | tee -a "$log"
exit "$status"
