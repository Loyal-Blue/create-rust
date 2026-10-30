"""Fix preview occlusion through the supported stock Horizon template API."""
from pathlib import Path
import json,zipfile,hashlib
root=Path(__file__).resolve().parents[1]
path=root/'create-horizon-content-preview.hmod'
solid={'andesite_alloy_block','brass_block','zinc_block','andesite_casing','brass_casing','copper_casing'}
with zipfile.ZipFile(path) as z:entries={n:z.read(n) for n in z.namelist()}
changed={}
for name in list(entries):
 if not name.startswith('data/create/horizon/block/') or not name.endswith('.json'):continue
 block=name.rsplit('/',1)[1][:-5];decl=json.loads(entries[name])
 if block in solid:continue
 old=decl['template']
 # Iron chains retain the axis property while supplying a non-occluding,
 # cutout template. Extra waterlogged states use the same axis variants.
 decl['template']='minecraft:iron_chain' if old in ('minecraft:basalt','minecraft:iron_chain') else 'minecraft:glass'
 entries[name]=(json.dumps(decl,indent=2)+'\n').encode()
 changed[block]={'old_template':old,'new_template':decl['template']}
manifest=entries['horizon-mod.toml'].decode().replace('version = "0.3.2"','version = "0.3.3"')
entries['horizon-mod.toml']=manifest.encode()
report=json.loads(entries['ASSET_REPORT.json']);report['version']='0.3.3'
report['occlusion_fix']={'changed_blocks':changed,'unchanged_solid_blocks':sorted(solid),'method':'Non-occluding vanilla templates via the stock content API; axis blocks use iron_chain, other shaped blocks use glass.','limits':['Collision shapes, render layer, lighting and waterlogging inherit the selected template and are not final Create behavior.','Not verified in a running game; no player-owned prepared resources available.']}
report['notes'].append('0.3.3: shaped blocks no longer inherit full-cube stone/basalt occlusion. Solid casings and material blocks retain their solid templates.')
entries['ASSET_REPORT.json']=(json.dumps(report,indent=2)+'\n').encode()
entries['INSTALL.txt']=b'''Create: Horizon Content Preview 0.3.3\nReplace the older create-horizon-content-preview.hmod in horizon-mods/.\nFully close and restart Horizon so the mod resource tables and chunk meshes rebuild.\nKeep only this version of the create content preview enabled.\nThis update fixes shaped blocks inheriting full-cube occlusion.\nSolid casings/material blocks remain solid; shaped parts use non-occluding templates.\nCollision, lighting, render layer and waterlogging still inherit vanilla template behavior.\nOriginal textures/models are preserved. This does not add working machines.\n'''
# Keep the bundled development rebuild from reintroducing the bad templates.
script='development/asset-work/port_original_assets.py'
s=entries[script].decode()
s=s.replace("'version':'0.3.2'","'version':'0.3.3'")
s=s.replace('version = "0.3.2"','version = "0.3.3"')
anchor="items={n.rsplit('/',1)[1][:-5]:json.loads(d) for n,d in original.items() if '/horizon/item/' in n}"
insert='''
solid_blocks={'andesite_alloy_block','brass_block','zinc_block','andesite_casing','brass_casing','copper_casing'}
for name,decl in blocks.items():
 if name not in solid_blocks:
  decl['template']='minecraft:iron_chain' if decl['template'] in ('minecraft:basalt','minecraft:iron_chain') else 'minecraft:glass'
  put_json(f'data/create/horizon/block/{name}.json',decl)
'''
assert anchor in s
s=s.replace(anchor,anchor+insert).replace("if decl['template']=='minecraft:basalt':","if decl['template'] in ('minecraft:basalt','minecraft:iron_chain'):")
entries[script]=s.encode()
# No visual resource should change as part of an occlusion-only fix.
with zipfile.ZipFile(path) as original:
 for n in original.namelist():
  if n.startswith('assets/'):assert entries[n]==original.read(n),n
assert len(changed)==28
for block,change in changed.items():
 states=json.loads(entries[f'assets/create/blockstates/{block}.json'])['variants']
 if change['new_template']=='minecraft:iron_chain':
  for axis in 'xyz':
   for waterlogged in ('false','true'):
    state={'axis':axis,'waterlogged':waterlogged}
    matches=[v for key,v in states.items() if all(state.get(k)==value for k,value in (part.split('=') for part in key.split(',') if part))]
    assert len(matches)==1,(block,state,matches)
 else:assert '' in states,(block,states)
with zipfile.ZipFile(path,'w') as z:
 for n,data in sorted(entries.items()):
  info=zipfile.ZipInfo(n,(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16;z.writestr(info,data)
for n,data in entries.items():
 if n.startswith(('assets/','data/')):
  p=root/'asset-work/result'/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
print(json.dumps({'blocks_fixed':len(changed),'templates':{template:sum(c['new_template']==template for c in changed.values()) for template in ['minecraft:iron_chain','minecraft:glass']},'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}))
