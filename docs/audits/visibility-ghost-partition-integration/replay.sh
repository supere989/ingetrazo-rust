#!/bin/bash
# Host runner: invoke with podman unshare bash replay.sh FULL_COMPILED_SHA FRESH_OUTPUT_DIR.
# Requires parent confirmation that both named binaries were built from that SHA.
set -euo pipefail
cleanup_owned_containers() {
 for name in visibility-ghost-integration-native visibility-ghost-integration-app; do
  timeout 8s podman stop --time 2 "$name" >/dev/null 2>&1 || true
  timeout 8s podman rm -f "$name" >/dev/null 2>&1 || true
 done
}
trap cleanup_owned_containers EXIT INT TERM
source_sha=${1:?full compiled source SHA required}
output=${2:?fresh absolute output directory required}
[[ "$source_sha" =~ ^[0-9a-f]{40}$ ]] || exit 2
[[ "$output" = /* && ! -e "$output" ]] || exit 2
mkdir -p "$output/native" "$output/application" "$output/input"
podman cp ingetrazo-publish:/tmp/ingetrazo-native-font-merge-target/debug/examples/native_vulkan_texture_draw_probe "$output/native-probe"
podman cp ingetrazo-publish:/tmp/ingetrazo-native-font-merge-target/debug/examples/native_vulkan_hidden_ghost_application_probe "$output/application-probe"
sha256sum "$output/native-probe" "$output/application-probe" > "$output/binary-sha256.txt"
printf '%s\n' "$source_sha" > "$output/compiled-source.txt"
merged=$(podman inspect ingetrazo-publish --format '{{.GraphDriver.Data.MergedDir}}')
image=$(podman inspect ingetrazo-publish --format '{{.Image}}')
python - "$merged/tmp/ingetrazo-next-integration/docs/audits/hidden-ghost-workflow/captures.zip" "$output/input" "$merged/tmp/ingetrazo-next-integration/docs/audits/hidden-ghost-workflow/manifest.json" <<'PYREF'
import sys,zipfile,json,hashlib
from pathlib import Path
manifest=json.loads(Path(sys.argv[3]).read_text())
assert hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()==manifest["archive_sha256"]
with zipfile.ZipFile(sys.argv[1]) as z:
 for name in z.namelist():
  if name.startswith('original/'):
   data=z.read(name)
   assert hashlib.sha256(data).hexdigest()==manifest['files'][name],name
   Path(sys.argv[2],name.removeprefix('original/')).write_bytes(data)
PYREF
printf '%s\n' '{"file_format_version":"1.0.0","ICD":{"library_path":"libEGL_nvidia.so.0","api_version":"1.4.303"}}' > "$output/native/nvidia.json"
printf '%s\n' '{"file_format_version":"1.0.0","ICD":{"library_path":"libGLX_nvidia.so.0","api_version":"1.4.303"}}' > "$output/application/nvidia-egl-icd.json"
timeout 70s podman run --rm --name visibility-ghost-integration-native --read-only --network none --device nvidia.com/gpu=0 --userns keep-id:uid=1000,gid=1000 --user 1000:1000 --security-opt label=disable --tmpfs /tmp:rw  -v /usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro -v /usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro  -v "$merged/tmp/ingetrazo-parity-python:/tmp/ingetrazo-parity-python:ro"  -v "$merged/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu:ro" -v "$merged/usr/bin:/usr/bin:ro" -v "$merged/usr/share:/usr/share:ro" -v "$merged/etc/alternatives:/etc/alternatives:ro" -v "$merged/etc/fonts:/etc/fonts:ro" -v "$merged/etc/ImageMagick-6:/etc/ImageMagick-6:ro"  -v "$merged/tmp/ingetrazo-next-integration/scripts:/probe:ro" -v "$output/native-probe":/app:ro -v "$output/input":/input:ro -v "$output/native":/out:rw  -e VK_DRIVER_FILES=/out/nvidia.json -e XDG_CACHE_HOME=/tmp/cache  "$image" /app /input /out > "$output/native-transport.log" 2>&1
timeout 70s podman run --rm --name visibility-ghost-integration-app --read-only --network none --device nvidia.com/gpu=0 --userns keep-id:uid=1000,gid=1000 --user 1000:1000 --security-opt label=disable --tmpfs /tmp:rw  -v /usr/lib/libEGL.so.1:/usr/lib/libEGL.so.1:ro -v /usr/lib/libGLdispatch.so.0:/usr/lib/libGLdispatch.so.0:ro  -v "$merged/tmp/ingetrazo-parity-python:/tmp/ingetrazo-parity-python:ro"  -v "$merged/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu:ro" -v "$merged/usr/bin:/usr/bin:ro" -v "$merged/usr/share:/usr/share:ro" -v "$merged/etc/alternatives:/etc/alternatives:ro" -v "$merged/etc/fonts:/etc/fonts:ro" -v "$merged/etc/ImageMagick-6:/etc/ImageMagick-6:ro"  -v "$merged/tmp/ingetrazo-next-integration/scripts:/probe:ro" -v "$output/application-probe":/app:ro -v "$output/input":/input:ro -v "$output/application":/out:rw  -e VK_DRIVER_FILES=/out/nvidia-egl-icd.json -e XDG_CACHE_HOME=/tmp/cache  "$image" /tmp/ingetrazo-parity-python/bin/python /probe/check-native-hidden-ghost-application.py /app /out --source "$source_sha" --scene /input/object-off.igz  > "$output/application-transport.log" 2>&1
