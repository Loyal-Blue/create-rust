#!/usr/bin/env python3
"""Build a NEW, isolated 26.3 Automation Lab world using pinned Horizon save APIs."""
import argparse,json,subprocess,tempfile,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--horizon',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1]
expected='47a3b39fa9e419ec03221fca5ef402d58e831f14'
marker=a.horizon/'.horizon-revision'
revision=marker.read_text().strip() if marker.is_file() else subprocess.check_output(['git','-C',str(a.horizon),'rev-parse','HEAD'],text=True).strip()
if revision!=expected:raise SystemExit(f'Horizon revision mismatch: expected {expected}, got {revision}')
with tempfile.TemporaryDirectory(prefix='create-lab-') as tmp:
 t=Path(tmp);(t/'src').mkdir();manifest='[package]\nname="create-automation-lab"\nversion="0.1.0"\nedition="2024"\n[dependencies]\n'
 for name,path in [('horizon-client',a.horizon/'crates/horizon-client'),('horizon-world',a.horizon/'crates/horizon-world'),('horizon-core',a.horizon/'crates/horizon-core'),('create-runtime',r/'crates/create-machines'),('create-simulation',r/'crates/create-recipes'),('create-horizon-core',r/'crates/create-core')]:manifest+=name+'={path='+json.dumps(str(path.resolve()))+(',default-features=false' if name=='horizon-client' else '')+'}\n'
 manifest+='serde_json="1"\n';(t/'Cargo.toml').write_text(manifest);(t/'src/main.rs').write_bytes((r/'tools/lab/world.rs').read_bytes())
 run=['cargo','run','--quiet','--manifest-path',str(t/'Cargo.toml'),'--'];subprocess.run(run+['catalog',str(t/'catalog.json')],check=True)
 subprocess.run(['python3',str(r/'tools/lab/plan.py'),'--catalog',str(t/'catalog.json'),'--output',str(t/'plan.json')],check=True)
 world=t/'Create Rust Automation Lab';subprocess.run(run+['build',str(t/'plan.json'),str(world)],check=True)
 plan=json.loads((t/'plan.json').read_text());guide=(r/'docs/TEST_LAB.md').read_text()+'\n\nBays in this generated world:\n\n'
 for c in plan['cases']:
  guide+=f"{c['id']}. {c['title']} — origin {c['origin']}. "+' '.join(c['expected'])+'\n'
 (world/'LAB_GUIDE.md').write_text(guide);a.output.parent.mkdir(parents=True,exist_ok=True)
 with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
  z.writestr('INSTALL_MAP.txt','For later testing: extract the Create Rust Automation Lab folder into the saves folder used by your Horizon installation. Keep level.dat directly inside that world folder. Requires the matching Create content .hmod and native support .hnmod. This is a development test world, not a complete Create port. Back up existing saves. No existing world is modified by this generator. Read LAB_GUIDE.md inside the world.\n')
  for f in sorted(world.rglob('*')):
   if f.is_file():z.write(f,str(f.relative_to(t)))
 with zipfile.ZipFile(a.output) as z:assert z.testzip() is None
 print(a.output)
