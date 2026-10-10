#!/usr/bin/env bash
# Virtual-display smoke only; does not operate the user's desktop.
set -euo pipefail
binary="${INGETRAZO_BINARY:-target/debug/ingetrazo}"
fixture="${1:-ingetrazo/examples/banca-pergola-yanque.igz}"
out="${2:-docs/audits}"
mkdir -p "$out"
binary=$(realpath "$binary")
fixture=$(realpath "$fixture")
out=$(realpath "$out")
xvfb-run -a -s "-screen 0 1280x900x24" bash -c '
set -euo pipefail
render_runtime=$(mktemp -d /tmp/ingetrazo-render-runtime.XXXXXX)
app_pid=""
cleanup() {
    if [[ -n "$app_pid" ]]; then kill "$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true; fi
    rm -rf -- "$render_runtime"
}
trap cleanup EXIT
export XDG_RUNTIME_DIR="$render_runtime"
export XDG_CONFIG_HOME="$render_runtime/config"
export XDG_DATA_HOME="$render_runtime/data"
WGPU_BACKEND=gl LIBGL_ALWAYS_SOFTWARE=1 "$1" "$2" > "$3/render-startup.log" 2>&1 &
app_pid=$!
# Wait for a rendered frame, not a fixed startup delay. Cold CI runners can
# spend longer than five seconds initializing software GL and loading textures.
colors=0
for attempt in $(seq 1 45); do
    if ! kill -0 "$app_pid" 2>/dev/null; then cat "$3/render-startup.log"; exit 1; fi
    import -window root "$3/render-pergola.png"
    colors=$(identify -format "%k" "$3/render-pergola.png")
    if ((colors >= 20)); then break; fi
    sleep 1
done
if ((colors < 20)); then cat "$3/render-startup.log"; echo "Rendered image is empty or nearly uniform after45 seconds"; exit 1; fi
printf "Virtual display startup and nonempty screenshot passed (%s colors). Visual inspection still required.\n" "$colors"
' _ "$binary" "$fixture" "$out"
