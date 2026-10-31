#!/usr/bin/env python3
"""Add local mechanical-crafter art; original model/texture bytes stay private."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--jar',type=Path,required=True);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
r=Path(__file__).resolve().parents[1];c=r/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
with zipfile.ZipFile(a.jar) as jar:
 names=set(jar.namelist());seen=set()
 def model(id):
  ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create' or id in seen:return
  seen.add(id);name=f'assets/create/models/{path}.json';v=json.loads(jar.read(name))
  if 'loader' in v:raise ValueError('Unsupported loader: '+id)
  entries[name]=jar.read(name)
  if 'parent' in v:model(v['parent'])
  for tex in v.get('textures',{}).values():
   if not tex.startswith('create:'):continue
   tex='assets/create/textures/'+tex.split(':',1)[1]+'.png';entries[tex]=jar.read(tex)
   if tex+'.mcmeta' in names:entries[tex+'.mcmeta']=jar.read(tex+'.mcmeta')
 model('create:item/mechanical_crafter');model('create:block/mechanical_crafter/block')
 def write(path,value):
  dest=c/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_text((json.dumps(value,separators=(',',':')) if path.endswith('support.json') else json.dumps(value,ensure_ascii=False) if path.endswith('en_us.json') else json.dumps(value,indent=2))+'\n')
 write('data/create/horizon/block/mechanical_crafter.json',{'template':'minecraft:end_rod','item':True,'properties':{'hardness':3.0,'sounds':'minecraft:copper_block','light':0}})
 write('assets/create/blockstates/mechanical_crafter.json',{'variants':{'facing='+face:{'model':'create:block/mechanical_crafter/item',**rot} for face,rot in [('north',{'x':90}),('east',{'x':90,'y':90}),('south',{'x':90,'y':180}),('west',{'x':90,'y':270}),('up',{}),('down',{'x':180})]}})
 write('assets/create/items/mechanical_crafter.json',{'model':{'type':'minecraft:model','model':'create:item/mechanical_crafter'}})
 config=json.loads((c/'data/create/create_preview/support.json').read_text());config['blocks']['create:mechanical_crafter']={face:[[0,0,0,1,1,1]] for face in ['north','east','south','west','up','down']}
 if not any(i['id']=='create:mechanical_crafter' for i in config['items']):config['items'].append({'id':'create:mechanical_crafter','name':'Mechanical Crafter'})
 write('data/create/create_preview/support.json',config)
 lang=json.loads((c/'assets/create/lang/en_us.json').read_text());lang['block.create.mechanical_crafter']='Mechanical Crafter';write('assets/create/lang/en_us.json',lang)
 write('data/create/loot_table/blocks/mechanical_crafter.json',{'type':'minecraft:block','pools':[{'rolls':1,'entries':[{'type':'minecraft:item','name':'create:mechanical_crafter'}]}]})
 tag=json.loads((c/'data/minecraft/tags/block/mineable/pickaxe.json').read_text());tag['values']=list(dict.fromkeys(tag['values']+['create:mechanical_crafter']));write('data/minecraft/tags/block/mineable/pickaxe.json',tag)
 def convert(v):
  if isinstance(v,list):return [convert(x) for x in v]
  if isinstance(v,dict):
   if 'item' in v:return v['item']
   if 'tag' in v:return '#'+v['tag']
  return v
 recipe=json.loads(jar.read('data/create/recipe/crafting/kinetics/mechanical_crafter.json'));recipe['key']={k:convert(v) for k,v in recipe['key'].items()};write('data/create/recipe/horizon_mechanical_crafter.json',recipe)
 recipe=json.loads(jar.read('data/create/recipe/mechanical_crafting/crushing_wheel.json'))
 assert all(row==row[::-1] for row in recipe['pattern']), 'Host shaped matcher mirrors; only a symmetric original pattern is supported here'
 recipe['type']='minecraft:crafting_shaped';recipe.pop('accept_mirrored',None);recipe.pop('show_notification',None);recipe['key']={k:convert(v) for k,v in recipe['key'].items()}
 recipe['key']['S']=['minecraft:stone','minecraft:andesite','minecraft:diorite','minecraft:granite','minecraft:deepslate','minecraft:tuff','minecraft:calcite']
 write('data/create/recipe/horizon_mechanical_crushing_wheel.json',recipe)
 for f in c.rglob('*'):
  if f.is_file():entries[str(f.relative_to(c))]=f.read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 for name,data in sorted(entries.items()):z.writestr(name,data)
print(a.output)
