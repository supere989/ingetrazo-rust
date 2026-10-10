#!/usr/bin/env python3
"""Focused actual Vk mutation controls; restores source even on failed admission."""
import os,subprocess,hashlib,json
from pathlib import Path
root=Path(__file__).resolve().parents[1]; out=root/'docs/audits/paper-canvas-gpu-lifetime';out.mkdir(exist_ok=True)
source=root/'crates/ingetrazo-gui/src/native_vulkan/mod.rs'; original=source.read_text();target=Path('/tmp/ingetrazo-geometry-paper-canvas-target');binary=target/'debug/examples/native_vulkan_raw_lifetime_probe'
env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
def build(name):
 with (out/(name+'-build.log')).open('w') as log: subprocess.run(['cargo','build','-j2','-p','ingetrazo-gui','--example','native_vulkan_raw_lifetime_probe'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
def run(name,needle):
 with (out/(name+'.log')).open('w') as log: result=subprocess.run(['timeout','30s','xvfb-run','-a','-s','-screen 0 800x600x24',str(binary)],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 text=(out/(name+'.log')).read_text();assert result.returncode!=0 and needle in text,(name,result.returncode,text)
 return dict(name=name,exit=result.returncode,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),required_rejection=needle)
records=[]
try:
 source.write_text(original.replace('        let dead=freed.iter().copied().filter(', '        return Ok(());\n        #[allow(unreachable_code)] let dead=freed.iter().copied().filter(',1));build('negative-no-cancel');records.append(run('negative-no-cancel','Released refusal uploaded orphan'))
 source.write_text(original.replace('        let dead=freed.iter().copied().filter(|id|!paint.iter().any(|p|matches!(&p.primitive,egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id==*id))).collect::<Vec<_>>();','        let dead=freed.to_vec();',1));build('negative-blanket-cancel');records.append(run('negative-blanket-cancel','Unexpected probe frame failure: Bad texture ID: Managed(3)'))
finally:
 source.write_text(original)
 build('restored')
(out/'mutations.json').write_text(json.dumps(dict(source_sha256=hashlib.sha256(original.encode()).hexdigest(),mutations=records,restored_binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest()),indent=2)+'\n')
