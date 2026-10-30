from pathlib import Path
import zipfile,json,itertools,math
import numpy as np
from scipy.spatial.transform import Rotation
root=Path(__file__).resolve().parents[1];path=root/'create-horizon-content-preview.hmod'
with zipfile.ZipFile(path) as z:e={n:z.read(n) for n in z.namelist()}
def put(n,v):e[n]=(json.dumps(v,separators=(',',':'))+'\n').encode()
def resolve(id):
 ns,p=(id if ':' in id else 'minecraft:'+id).split(':');n=f'assets/{ns}/models/{p}.json'
 if n not in e:return {}
 m=json.loads(e[n]);r=resolve(m['parent']) if 'parent' in m else {};r.update(m);return r
solid={'andesite_alloy_block','brass_block','zinc_block','andesite_casing','brass_casing','copper_casing'}
upright={'mechanical_press','mechanical_mixer','millstone','hand_crank','deployer','mechanical_saw'}
axisz={'creative_motor','clutch','gearshift','encased_chain_drive','encased_fan','mechanical_drill','mechanical_pump'}
config={'schema':1,'blocks':{},'items':[]}
lang=json.loads(e['assets/create/lang/en_us.json'])
for n in sorted(e.copy()):
 if '/horizon/block/' not in n:continue
 name=n.split('/')[-1][:-5];statepath=f'assets/create/blockstates/{name}.json';states=json.loads(e[statepath]);variants=states['variants'];model=next(iter(variants.values()))['model']
 if name in upright:
  variants={k:{'model':model} for k in variants}
 elif name in axisz:
  variants={'axis=z':{'model':model},'axis=x':{'model':model,'y':90},'axis=y':{'model':model,'x':90}}
 states['variants']=variants;put(statepath,states)
 boxes=[]
 for el in resolve(model).get('elements',[]):
  points=np.array(list(itertools.product(*zip(el['from'],el['to']))),float)
  if any(abs(el['to'][a]-el['from'][a])<1e-5 for a in range(3)):continue
  rot=el.get('rotation',{});angle=rot.get('angle',0)
  if angle:
   origin=np.array(rot.get('origin',[8,8,8]));points=Rotation.from_euler(rot['axis'],angle,degrees=True).apply(points-origin)+origin
  b=np.r_[points.min(0),points.max(0)]/16;b=np.clip(np.round(b*16)/16,0,1)
  if all(b[a]<b[a+3] for a in range(3)):boxes.append(b.tolist())
 if name in solid or not boxes:boxes=[[0,0,0,1,1,1]]
 if name in {'water_wheel','large_water_wheel','crushing_wheel'}:boxes=[[0,0.25,0,1,0.75,1],[0.375,0,0.375,0.625,1,0.625]]
 boxes=list(map(list,dict.fromkeys(map(tuple,boxes))))
 out={}
 for selector,v in variants.items():
  result=[]
  for b in boxes:
   points=np.array(list(itertools.product(*zip(b[:3],b[3:]))))-.5
   for a in 'xyz':
    if v.get(a):points=Rotation.from_euler(a,-v[a],degrees=True).apply(points)
   points+=.5;result.append(np.round(np.r_[points.min(0),points.max(0)],6).tolist())
  out[selector.split('=')[-1] if selector else '']=result
 config['blocks']['create:'+name]=out
for n in sorted(e):
 if '/horizon/block/' in n or '/horizon/item/' in n:
  name=n.split('/')[-1][:-5];config['items'].append({'id':'create:'+name,'name':lang.get('block.create.'+name,lang.get('item.create.'+name,name.replace('_',' ').title()))})
put('data/create/create_preview/support.json',config)
e['horizon-mod.toml']=e['horizon-mod.toml'].replace(b'0.3.3',b'0.3.4')
e['INSTALL.txt']=b'''Create Content Preview 0.3.4\nReplace the previous content .hmod. Keep only one copy enabled.\nFor corrected collision/selection shapes and the Create creative tab, also install\ncreate-preview-support.hnmod in the SAME launcher instance and use Native Mods > Build.\nThen start the native build with this content pack enabled.\nThe tab appears to the left of the top row and contains all 59 registered items.\nRestart completely after updating.\nShapes approximate the static models using box unions. Existing axis properties are preserved.\nMachines remain static; this is not a complete working Create port.\n'''
report=json.loads(e['ASSET_REPORT.json']);report['version']='0.3.4';report['shape_update']={'native_companion_required':True,'blocks':34,'tab_items':59,'method':'Box unions from static model elements, rounded to 1/16; rotated elements use enclosing boxes; large wheels use bounded approximations.','limitations':['Not original Create shape parity','No working machine logic','No in-game verification']};put('ASSET_REPORT.json',report)
with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as z:
 for n,d in sorted(e.items()):z.writestr(n,d)
for n,d in e.items():
 if n.startswith(('assets/','data/')):
  p=root/'asset-work/result'/n;p.parent.mkdir(exist_ok=True,parents=True);p.write_bytes(d)
print('Updated 34 blocks, 59 tab items')
