#!/usr/bin/env python3
"""Package source only. Installation/build trust remains a launcher action."""
import argparse,json,zipfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,default=Path('dist/create-preview-support.hnmod'));a=p.parse_args()
r=Path(__file__).resolve().parents[1];base=r/'native';m=base/'mixins/create_preview_support'
from embed_runtime import generate
generate(r)
manifest={'schema':1,'id':'create_preview_support','version':'0.3.1','game':'26.3','horizon':'47a3b39fa9e419ec03221fca5ef402d58e831f14','mixins':'cb1ad23022392d88f006cf94cbf6552a73f5df12'}
a.output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(a.output,'w',zipfile.ZIP_DEFLATED) as z:
 z.writestr('horizon-native.json',json.dumps(manifest,indent=2)+'\n')
 for f in sorted(m.iterdir()):
  if f.suffix in ('.rs','.toml'):z.write(f,str(f.relative_to(base)))
print(a.output)
