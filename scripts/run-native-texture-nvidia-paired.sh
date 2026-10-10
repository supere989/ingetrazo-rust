#!/usr/bin/env bash
# Host transport only. All source/renderer execution remains in isolated devboxes.
set -euo pipefail
publish=${INGETRAZO_PUBLISH_CONTAINER:-ingetrazo-publish}
worktree=${INGETRAZO_TEXTURE_WORKTREE:-/tmp/ingetrazo-textures}
: "${DISPLAY:?Existing authorized X display required}"
: "${XAUTHORITY:?Existing authorization file required}"
: "${FROZEN_PROBE:?Readonly frozen8f14 binary transport path required}"
: "${CORRECTED_PROBE:?Readonly a60d92e binary transport path required}"
: "${OUTPUT:?Fresh output directory required}"
[[ ! -e "$OUTPUT" ]] || { echo 'Refusing existing output directory' >&2; exit 1; }
mkdir -m700 "$OUTPUT"
merged=$(podman inspect "$publish" --format '{{.GraphDriver.Data.MergedDir}}')
image=$(podman inspect "$publish" --format '{{.Image}}')
source_host=$(podman inspect "$publish" --format '{{range .Mounts}}{{if eq .Destination "/work"}}{{.Source}}{{end}}{{end}}')
[[ $(podman exec "$publish" git -C /work/ingetrazo rev-parse HEAD) == 6be29fe437a7cd992d4e079f71720821b25216b9 ]] || { echo 'Wrong original source commit' >&2; exit 1; }
[[ -r "$XAUTHORITY" && -x "$FROZEN_PROBE" && -x "$CORRECTED_PROBE" ]]
name="ingetrazo-texture-pair-$$"
trap 'podman rm --force "$name" >/dev/null 2>&1 || true' EXIT
common=(--rm --name "$name" --read-only --network none --device nvidia.com/gpu=0 --userns keep-id:uid=1000,gid=1000 --user 1000:1000 --security-opt label=disable --tmpfs /tmp:rw)
gpu_libs=(-v /usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro -v /usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro)
for pair in 'five reference-native-texture-gl.py' 'heldout reference-native-texture-heldout.py'; do
 read -r corpus helper <<<"$pair"
 timeout 60s podman run "${common[@]}" "${gpu_libs[@]}"   -v "$merged/tmp/ingetrazo-parity-python:/tmp/ingetrazo-parity-python:ro"   -v "$merged/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu:ro"   -v "$merged$worktree/scripts:/probe:ro" -v "$source_host/ingetrazo:/source/ingetrazo:ro"   -v "$OUTPUT:/out:rw" -v /tmp/.X11-unix:/tmp/.X11-unix:ro -v "$XAUTHORITY:/auth:ro"   -e "DISPLAY=$DISPLAY" -e XAUTHORITY=/auth -e QT_QPA_PLATFORM=xcb -e QT_OPENGL=desktop -e INGETRAZO_REQUIRE_NVIDIA=1   -w /source "$image" /tmp/ingetrazo-parity-python/bin/python "/probe/$helper" "/out/original-$corpus"   >"$OUTPUT/original-$corpus.log" 2>&1
 for candidate in frozen corrected; do
  probe=$FROZEN_PROBE; [[ $candidate != corrected ]] || probe=$CORRECTED_PROBE
  out="$OUTPUT/native-$corpus-$candidate";mkdir -m700 "$out"
  cat >"$out/nvidia-egl-icd.json" <<'JSON'
{"file_format_version":"1.0.0","ICD":{"library_path":"/usr/lib/libEGL_nvidia.so.0","api_version":"1.4.351"}}
JSON
  timeout 50s podman run "${common[@]}" "${gpu_libs[@]}" -v "$probe:/probe:ro"    -v "$OUTPUT/original-$corpus:/input:ro" -v "$out:/out:rw" -e VK_DRIVER_FILES=/out/nvidia-egl-icd.json    "$image" /probe /input /out >"$OUTPUT/native-$corpus-$candidate.log" 2>&1
 done
done
sha256sum "$FROZEN_PROBE" "$CORRECTED_PROBE" >"$OUTPUT/binary-sha256.txt"
# Deliberately does not claim checker success: full heldout still fails Shaded.
# Copy output into publish and run strict check-native-texture-{draw,heldout}.py.
