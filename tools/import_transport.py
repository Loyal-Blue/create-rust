#!/usr/bin/env python3
"""Import local Create transport art. Original assets stay in the private output archive."""
import argparse, copy, itertools, json, math, zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--jar',type=Path,required=True)
p.add_argument('--assets-from',type=Path,required=True)
p.add_argument('--output',type=Path,required=True)
a=p.parse_args();root=Path(__file__).resolve().parents[1];content=root/'content'
with zipfile.ZipFile(a.assets_from) as z: entries={n:z.read(n) for n in z.namelist()}
with zipfile.ZipFile(a.jar) as jar:
 names=set(jar.namelist());seen=set()
 def texture(id):
  if id.startswith('#'):return
  ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create':return
  name=f'assets/create/textures/{path}.png';entries[name]=jar.read(name)
  if name+'.mcmeta' in names:entries[name+'.mcmeta']=jar.read(name+'.mcmeta')
 def model(id):
  ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create' or id in seen:return
  seen.add(id);name=f'assets/create/models/{path}.json';value=json.loads(jar.read(name))
  if 'loader' in value:raise ValueError('Unsupported custom model: '+id)
  entries[name]=jar.read(name)
  if 'parent' in value:model(value['parent'])
  for tex in value.get('textures',{}).values():texture(tex)
 for name in ['andesite_funnel','brass_funnel']:
  model('create:item/'+name)
  for mode in ['pull','push']:
   for power in ['unpowered','powered']:model(f'create:block/{name}_horizontal_{mode}_{power}')
 model('create:item/belt_connector')
 for tex in ['create:block/belt','create:block/axis','create:block/axis_top']:texture(tex)
 def resolve(id):
  ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1);name=f'assets/{ns}/models/{path}.json'
  if name not in entries:return {}
  v=json.loads(entries[name]);base=resolve(v['parent']) if 'parent' in v else {};textures={**base.get('textures',{}),**v.get('textures',{})};base.update(v);base['textures']=textures;return base
 def rotate(point,axis,angle,origin=(8,8,8)):
  x=[point[i]-origin[i] for i in range(3)];a='xyz'.index(axis);u=(a+1)%3;v=(a+2)%3;c=math.cos(math.radians(angle));s=math.sin(math.radians(angle));x[u],x[v]=x[u]*c-x[v]*s,x[u]*s+x[v]*c;return [x[i]+origin[i] for i in range(3)]
 configpath=content/'data/create/create_preview/support.json';config=json.loads(configpath.read_text());langpath=content/'assets/create/lang/en_us.json';lang=json.loads(langpath.read_text())
 faces={'north':0,'east':90,'south':180,'west':270,'up':0,'down':0}
 belt=json.loads((content/'assets/create/models/block/horizon_transport/belt.json').read_text())
 for name in ['belt','andesite_funnel','brass_funnel']:
  if name=='belt': base='create:block/horizon_transport/belt';value=belt
  else:base=f'create:block/{name}_horizontal_pull_unpowered';value=resolve(base)
  variants={};shapes={}
  for face,y in faces.items():
   variants['facing='+face]={'model':base,**({'y':y} if y else {})};boxes=[]
   for el in value.get('elements',[]):
    points=list(itertools.product(*zip(el['from'],el['to'])))
    if 'rotation' in el:
     rot=el['rotation'];points=[rotate(q,rot.get('axis','y'),rot.get('angle',0),rot.get('origin',[8,8,8])) for q in points]
    points=[rotate(q,'y',-y) for q in points]
    box=[max(0,min(1,min(q[i] for q in points)/16)) for i in range(3)]+[max(0,min(1,max(q[i] for q in points)/16)) for i in range(3)]
    if all(box[i]<box[i+3] for i in range(3)):boxes.append(box)
   shapes[face]=boxes
  config['blocks']['create:'+name]=shapes
  (content/f'assets/create/blockstates/{name}.json').write_text(json.dumps({'variants':variants})+'\n')
  (content/f'data/create/horizon/block/{name}.json').write_text(json.dumps({'template':'minecraft:end_rod','item':True,'properties':{'hardness':3.0,'sounds':'minecraft:copper_block','light':0}})+'\n')
  (content/f'assets/create/items/{name}.json').write_text(json.dumps({'model':{'type':'minecraft:model','model':base if name=='belt' else 'create:item/'+name}})+'\n')
  display='Mechanical Belt' if name=='belt' else name.replace('_',' ').title();lang['block.create.'+name]=display
  if not any(i['id']=='create:'+name for i in config['items']):config['items'].append({'id':'create:'+name,'name':display})
 (content/'data/create/horizon/item/belt_connector.json').write_text(json.dumps({'template':'minecraft:iron_ingot','max_stack_size':64})+'\n')
 (content/'assets/create/items/belt_connector.json').write_text(json.dumps({'model':{'type':'minecraft:model','model':'create:item/belt_connector'}})+'\n')
 lang['item.create.belt_connector']='Belt Connector'
 if not any(i['id']=='create:belt_connector' for i in config['items']):config['items'].append({'id':'create:belt_connector','name':'Belt Connector'})
 configpath.write_text(json.dumps(config,separators=(',',':'))+'\n');langpath.write_text(json.dumps(lang,ensure_ascii=False)+'\n')
 tagpath=content/'data/minecraft/tags/block/mineable/pickaxe.json';tag=json.loads(tagpath.read_text());tag['values']=list(dict.fromkeys(tag['values']+['create:belt','create:andesite_funnel','create:brass_funnel']));tagpath.write_text(json.dumps(tag,indent=2)+'\n')
for f in content.rglob('*'):
 if f.is_file():entries[str(f.relative_to(content))]=f.read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as out:
 for name,data in sorted(entries.items()):out.writestr(name,data)
print(a.output)
