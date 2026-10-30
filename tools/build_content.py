#!/usr/bin/env python3
"""Build the existing data-only preview; never packages executable mod code."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--assets-from',type=Path,required=True)
p.add_argument('--output',type=Path,default=Path('dist/create.hmod'))
a=p.parse_args();root=Path(__file__).resolve().parents[1];entries={}
with zipfile.ZipFile(a.assets_from) as source:
 for name in source.namelist():
  if name.startswith('assets/create/') and not any(v in ('','..','.') for v in name.split('/')):
   entries[name]=source.read(name)
for f in (root/'content').rglob('*'):
 if f.is_file():entries[str(f.relative_to(root/'content'))]=f.read_bytes()
if not any(n.startswith('assets/create/models/') for n in entries):raise SystemExit('Input contains no Create model assets')
entries['INSTALL.txt']=b'Data-only static content preview. No working rotation or inventory pagination. Enable mod.create:true and restart.\n'
entries['NOTICE.md']=(root/'NOTICE.md').read_bytes()
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as out:
 for name,data in sorted(entries.items()):
  info=zipfile.ZipInfo(name,(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16;out.writestr(info,data)
print(json.dumps({'output':str(a.output),'files':len(entries),'runtime':'data-only static preview'}))
