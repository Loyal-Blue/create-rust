#!/usr/bin/env python3
"""Split private normalized assemblies for native partial-model animation."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--assets-from',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();root=Path(__file__).resolve().parents[1];content=root/'content'
with zipfile.ZipFile(a.assets_from) as z:entries={n:z.read(n) for n in z.namelist()}
for machine in ['mechanical_press','mechanical_mixer']:
 full=json.loads(entries[f'assets/create/models/block/horizon_runtime/{machine}.json']);parts={}
 for el in full['elements']:
  name=el.get('name','').lower()
  if machine=='mechanical_press':part='press_shaft' if name=='axis' else 'press_head' if name.startswith('pole') or name=='head' else 'press_body'
  else:part='mixer_head' if name.startswith('mixer') else 'mixer_pole' if name.startswith('pole') else 'mixer_gear' if name.startswith('gear') else 'mixer_body'
  parts.setdefault(part,[]).append(el)
 for part,elements in parts.items():
  data={'ambientocclusion':False,'textures':full.get('textures',{}),'elements':elements}
  entries[f'assets/create/models/block/horizon_parts/{part}.json']=(json.dumps(data,separators=(',',':'))+'\n').encode()
  if not part.endswith('_body'):
   file=content/f'assets/create/items/horizon_{part}.json';file.write_text(json.dumps({'model':{'type':'minecraft:model','model':'create:block/horizon_parts/'+part}})+'\n')
 for part in (['press_body','press_head','press_shaft'] if machine=='mechanical_press' else ['mixer_body','mixer_head','mixer_pole','mixer_gear']):assert part in parts,part
for file in content.rglob('*'):
 if file.is_file():entries[str(file.relative_to(content))]=file.read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 for name,data in sorted(entries.items()):z.writestr(name,data)
print(a.output)
