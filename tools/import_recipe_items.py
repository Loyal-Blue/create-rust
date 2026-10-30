#!/usr/bin/env python3
"""Import additional non-block recipe item art from a local Create JAR. Do not redistribute original art."""
import argparse,copy,json,re,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--jar',type=Path,required=True);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
r=Path(__file__).resolve().parents[1];content=r/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
with zipfile.ZipFile(a.jar) as jar:
 names=set(jar.namelist());lang=json.loads(jar.read('assets/create/lang/en_us.json'));ids={f.stem for d in ['block','item'] for f in (content/f'data/create/horizon/{d}').glob('*.json')};new=set()
 for part in (r/'crates/create-recipes/src/catalog.rs').read_text().split('Recipe {')[1:]:
  for id in re.findall(r'(?:id|matcher):\s*"create:([^"]+)"',part):
   if id not in ids and f'assets/create/models/item/{id}.json' in names and f'assets/create/blockstates/{id}.json' not in names:new.add(id)
 def texture(id):
  if id.startswith('#'):return
  ns,id=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create':return
  path=f'assets/create/textures/{id}.png';entries[path]=jar.read(path)
  if path+'.mcmeta' in names:entries[path+'.mcmeta']=jar.read(path+'.mcmeta')
 seen=set()
 def model(id):
  ns,id=(id if ':' in id else 'minecraft:'+id).split(':',1)
  if ns!='create' or id in seen:return
  seen.add(id);path=f'assets/create/models/{id}.json';value=json.loads(jar.read(path))
  if value.get('loader'):raise ValueError('Use the original conversion pipeline for custom-loaded item '+id)
  entries[path]=jar.read(path)
  if 'parent' in value:model(value['parent'])
  for value in value.get('textures',{}).values():texture(value)
 configpath=content/'data/create/create_preview/support.json';config=json.loads(configpath.read_text());translations=json.loads((content/'assets/create/lang/en_us.json').read_text())
 for id in sorted(new):
  model('create:item/'+id);food=id in {'bar_of_chocolate','honeyed_apple','chocolate_glazed_berries'};template='minecraft:cookie' if food else 'minecraft:honey_bottle' if id=='builders_tea' else 'minecraft:iron_ingot'
  f=content/f'data/create/horizon/item/{id}.json';f.write_text(json.dumps({'template':template,'max_stack_size':64})+'\n')
  f=content/f'assets/create/items/{id}.json';f.write_text(json.dumps({'model':{'type':'minecraft:model','model':'create:item/'+id}})+'\n')
  display=lang.get('item.create.'+id,id.replace('_',' ').title());translations['item.create.'+id]=display;config['items'].append({'id':'create:'+id,'name':display})
 (content/'assets/create/lang/en_us.json').write_text(json.dumps(translations,ensure_ascii=False)+'\n');configpath.write_text(json.dumps(config,separators=(',',':'))+'\n')
manifest=content/'horizon-mod.toml';manifest.write_text(manifest.read_text().replace('version = "0.4.0"','version = "0.5.0"'))
for f in content.rglob('*'):
 if f.is_file():entries[str(f.relative_to(content))]=f.read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as out:
 for n,data in sorted(entries.items()):out.writestr(n,data)
print(json.dumps({'new_items':sorted(new),'items_in_tab':len(config['items']),'output':str(a.output)}))
