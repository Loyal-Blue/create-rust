#!/usr/bin/env python3
"""Normalize local item assemblies into placed machine models and matching box unions."""
import argparse,copy,itertools,json,zipfile
from pathlib import Path
import numpy as np
from scipy.spatial.transform import Rotation
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1];content=r/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
def resolve(id):
 ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1);file=f'assets/{ns}/models/{path}.json'
 if file not in entries:return {}
 model=json.loads(entries[file]);parent=resolve(model['parent']) if 'parent' in model else {};textures=parent.get('textures',{}).copy();textures.update(model.get('textures',{}));parent.update(model);parent['textures']=textures;return parent
south={'south':{},'north':{'y':180},'east':{'y':270},'west':{'y':90},'up':{'x':90},'down':{'x':270}}
north={'north':{},'south':{'y':180},'east':{'y':90},'west':{'y':270},'up':{'x':270},'down':{'x':90}}
east={'east':{},'west':{'y':180},'south':{'y':90},'north':{'y':270},'up':{'z':270},'down':{'z':90}}
configpath=content/'data/create/create_preview/support.json';config=json.loads(configpath.read_text())
for name in ['creative_motor','encased_fan','mechanical_drill','deployer','mechanical_pump','hand_crank','mechanical_saw','mechanical_press','mechanical_mixer','millstone']:
 model=resolve('create:item/'+name);delta=np.array([0,-4 if name=='mechanical_press' else -7 if name=='mechanical_mixer' else 0,0]);es=copy.deepcopy(model.get('elements',[]))
 for element in es:
  for key in ['from','to']:element[key]=(np.array(element[key])+delta).tolist()
  if 'rotation' in element:element['rotation']['origin']=(np.array(element['rotation'].get('origin',[8,8,8]))+delta).tolist()
 generated={'textures':model.get('textures',{}),'elements':es,'ambientocclusion':False};path=f'assets/create/models/block/horizon_runtime/{name}.json';entries[path]=(json.dumps(generated,separators=(',',':'))+'\n').encode()
 variants=north if name in ['encased_fan','hand_crank','mechanical_press'] else east if name=='mechanical_pump' else south
 if name in ['mechanical_mixer','millstone']:variants={f:{} for f in south}
 if name=='mechanical_press':variants={**north,'up':{},'down':{}}
 worlds={};shapes={}
 for facing,rot in variants.items():
  worlds['facing='+facing]={'model':'create:block/horizon_runtime/'+name,**rot};boxes=[]
  for el in es:
   if any(abs(el['to'][i]-el['from'][i])<1e-5 for i in range(3)):continue
   points=np.array(list(itertools.product(*zip(el['from'],el['to']))),float)
   er=el.get('rotation',{});origin=np.array(er.get('origin',[8,8,8]));angles=[er.get(ax,0) for ax in 'xyz']
   if 'axis' in er:angles['xyz'.index(er['axis'])]=er.get('angle',0)
   points=Rotation.from_euler('xyz',angles,degrees=True).apply(points-origin)+origin
   for ax in 'xyz':
    if ax in rot:points=Rotation.from_euler(ax,-rot[ax],degrees=True).apply(points-8)+8
   b=np.clip(np.round(np.r_[points.min(0),points.max(0)])/16,0,1)
   if all(b[i]<b[i+3] for i in range(3)):boxes.append(b.tolist())
  shapes[facing]=list(map(list,dict.fromkeys(map(tuple,boxes)))) or [[0,0,0,1,1,1]]
 (content/f'assets/create/blockstates/{name}.json').write_text(json.dumps({'variants':worlds})+'\n');config['blocks']['create:'+name]=shapes
configpath.write_text(json.dumps(config,separators=(',',':'))+'\n')
for f in content.rglob('*'):
 if f.is_file():entries[str(f.relative_to(content))]=f.read_bytes()
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 for n,data in sorted(entries.items()):z.writestr(n,data)
print(a.output)
