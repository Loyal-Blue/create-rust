from pathlib import Path
import json,itertools
import numpy as np
from scipy.spatial.transform import Rotation
root=Path(__file__).resolve().parents[1];content=root/'content';
if '0.4.0' in (content/'horizon-mod.toml').read_text(): raise SystemExit('Content already upgraded; no changes made')
p=content/'data/create/create_preview/support.json';config=json.loads(p.read_text())
directional={'creative_motor','encased_fan','mechanical_drill','deployer','mechanical_pump'}
upright={'mechanical_press','mechanical_mixer','mechanical_saw','millstone','hand_crank'}
rotations={'south':{},'north':{'y':180},'east':{'y':270},'west':{'y':90},'up':{'x':90},'down':{'x':270}}
for name in sorted(directional|upright):
 decl=content/f'data/create/horizon/block/{name}.json';d=json.loads(decl.read_text());d['template']='minecraft:end_rod';d.setdefault('properties',{})['light']=0;decl.write_text(json.dumps(d,indent=2)+'\n')
 states=content/f'assets/create/blockstates/{name}.json';v=json.loads(states.read_text())['variants'];model=next(iter(v.values()))['model']
 source=config['blocks']['create:'+name];boxes=source.get('z',source.get('y',source.get('',next(iter(source.values())))))
 out={};variants={}
 for face,rot in rotations.items():
  variants['facing='+face]={'model':model,**rot};out[face]=[]
  for b in boxes:
   points=np.array(list(itertools.product(*zip(b[:3],b[3:]))))-.5
   for a in 'xyz':
    if a in rot:points=Rotation.from_euler(a,-rot[a],degrees=True).apply(points)
   points+=.5;out[face].append(np.round(np.r_[points.min(0),points.max(0)],6).tolist())
 states.write_text(json.dumps({'variants':variants})+'\n');config['blocks']['create:'+name]=out
p.write_text(json.dumps(config,separators=(',',':'))+'\n')
manifest=content/'horizon-mod.toml';manifest.write_text(manifest.read_text().replace('0.3.4','0.4.0'))
