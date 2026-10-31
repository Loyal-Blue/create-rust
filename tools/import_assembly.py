#!/usr/bin/env python3
"""Import two assembly models/sprites into a private local content build."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--jar',type=Path,required=True);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1];c=r/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
config=json.loads((c/'data/create/create_preview/support.json').read_text());lang=json.loads((c/'assets/create/lang/en_us.json').read_text())
def write(path,value):
 dest=c/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_text((json.dumps(value,separators=(',',':')) if path.endswith('support.json') else json.dumps(value,ensure_ascii=False) if path.endswith('en_us.json') else json.dumps(value,indent=2))+'\n')
with zipfile.ZipFile(a.jar) as jar:
 names=json.loads(jar.read('assets/create/lang/en_us.json'))
 for name in ['precision_mechanism','incomplete_precision_mechanism']:
  model='assets/create/models/item/'+name+'.json';v=json.loads(jar.read(model));assert v['parent']=='minecraft:item/generated';entries[model]=jar.read(model)
  for texture in v['textures'].values():
   assert texture.startswith('create:');path='assets/create/textures/'+texture.split(':',1)[1]+'.png';entries[path]=jar.read(path)
  write('data/create/horizon/item/'+name+'.json',{'template':'minecraft:iron_ingot',**({'max_stack_size':1} if name.startswith('incomplete_') else {})})
  write('assets/create/items/'+name+'.json',{'model':{'type':'minecraft:model','model':'create:item/'+name}})
  id='create:'+name;label=names['item.create.'+name];lang['item.create.'+name]=label
  if not any(i['id']==id for i in config['items']):config['items'].append({'id':id,'name':label})
 write('data/create/create_preview/support.json',config);write('assets/create/lang/en_us.json',lang)
 for f in c.rglob('*'):
  if f.is_file():entries[str(f.relative_to(c))]=f.read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 for name,data in sorted(entries.items()):z.writestr(name,data)
print(a.output)
