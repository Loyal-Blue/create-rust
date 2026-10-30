#!/usr/bin/env python3
"""Add local ore/ingredient assets; original artwork stays out of the public repository."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--jar',type=Path,required=True);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1];root=r/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
with zipfile.ZipFile(a.jar) as jar:
 names=set(jar.namelist());lang=json.loads(jar.read('assets/create/lang/en_us.json'))
 def model(id):
  ns,path=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create':return
  path=f'assets/create/models/{path}.json';data=jar.read(path);entries[path]=data;m=json.loads(data)
  if 'parent' in m:model(m['parent'])
  for tex in m.get('textures',{}).values():
   if tex.startswith('#')or not tex.startswith('create:'):continue
   path='assets/create/textures/'+tex.split(':',1)[1]+'.png';entries[path]=jar.read(path)
 configp=root/'data/create/create_preview/support.json';config=json.loads(configp.read_text());lp=root/'assets/create/lang/en_us.json';language=json.loads(lp.read_text())
 def put(path,value):
  f=root/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_text(json.dumps(value,ensure_ascii=False)+'\n')
 for name in ['zinc_ore','deepslate_zinc_ore']:
  model('create:block/'+name);put(f'data/create/horizon/block/{name}.json',{'template':'minecraft:stone','properties':{'hardness':4.5 if name.startswith('deepslate')else 3.0,'requires_tool':True},'item':True});put(f'assets/create/blockstates/{name}.json',{'variants':{'':{'model':'create:block/'+name}}});put(f'assets/create/items/{name}.json',{'model':{'type':'minecraft:model','model':'create:block/'+name}});put(f'data/create/loot_table/blocks/{name}.json',{'type':'minecraft:block','pools':[{'rolls':1,'entries':[{'type':'minecraft:item','name':'create:raw_zinc'}],'conditions':[{'condition':'minecraft:survives_explosion'}]}]});config['blocks']['create:'+name]={'':[[0,0,0,1,1,1]]};title=lang.get('block.create.'+name,name.replace('_',' ').title());language['block.create.'+name]=title
  if not any(i['id']=='create:'+name for i in config['items']):config['items'].append({'id':'create:'+name,'name':title})
 model('create:item/raw_zinc');put('data/create/horizon/item/raw_zinc.json',{'template':'minecraft:raw_iron'});put('assets/create/items/raw_zinc.json',{'model':{'type':'minecraft:model','model':'create:item/raw_zinc'}});language['item.create.raw_zinc']=lang.get('item.create.raw_zinc','Raw Zinc')
 if not any(i['id']=='create:raw_zinc' for i in config['items']):config['items'].append({'id':'create:raw_zinc','name':language['item.create.raw_zinc']})
 for kind,time in [('smelting',200),('blasting',100)]:put(f'data/create/recipe/{kind}/raw_zinc.json',{'type':'minecraft:'+kind,'ingredient':'create:raw_zinc','result':{'id':'create:zinc_ingot'},'experience':0.7,'cookingtime':time})
 for tag in ['mineable/pickaxe','needs_stone_tool']:
  f=root/f'data/minecraft/tags/block/{tag}.json';d=json.loads(f.read_text()) if f.exists()else{'values':[]};d['values']=list(dict.fromkeys(d.get('values',[])+['create:zinc_ore','create:deepslate_zinc_ore']));put(str(f.relative_to(root)),d)
 lp.write_text(json.dumps(language,ensure_ascii=False)+'\n');configp.write_text(json.dumps(config,separators=(',',':'))+'\n')
for f in root.rglob('*'):
 if f.is_file():entries[str(f.relative_to(root))]=f.read_bytes()
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 for n,d in sorted(entries.items()):z.writestr(n,d)
print(a.output)
