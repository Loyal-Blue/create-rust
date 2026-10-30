#!/usr/bin/env python3
"""Private asset-format adaptation from the user's own Create JAR.
Original JSON/PNG are retained where compatible. NeoForge OBJ meshes are
compiled into textured native model planes; no Java code is loaded or ported.
"""
from pathlib import Path
import copy,hashlib,io,json,math,warnings,zipfile
import numpy as np
from obj_vanilla import convert as convert_vanilla
from native_plane import native_plane
from merge_obj import merge_rectangles
from PIL import Image
from scipy.spatial.transform import Rotation
ROOT=Path(__file__).resolve().parents[1]
JAR=ROOT/'asset-inputs/create-1.21.1-6.0.10.jar'
BASE=ROOT/'asset-inputs/create-horizon-content-preview.hmod'
OUT=ROOT/'create-horizon-content-preview.hmod'
jar=zipfile.ZipFile(JAR)
with zipfile.ZipFile(BASE) as z:original={n:z.read(n) for n in z.namelist()}
entries={n:d for n,d in original.items() if not n.startswith('assets/')}
source_models={}
for n in jar.namelist():
 if n.startswith('assets/create/models/') and n.endswith('.json'):
  source_models['create:'+n[len('assets/create/models/'):-5]]=json.loads(jar.read(n))
models={};visiting=set();pngs={};converted={};copied_textures=set();resampled={}
report={'source_jar_sha256':hashlib.sha256(JAR.read_bytes()).hexdigest(),'version':'0.3.2','item_ids':[],'static_block_ids':[],'obj_conversions':{},'notes':['Original Create textures and item models from the user-provided JAR.','OBJ materials are resampled onto transparent native model planes.','Placed machines use static assemblies; original animations and behavior are not implemented.','Original PNGs are copied byte-for-byte; generated OBJ face patches sample their original materials.']}
def ident(s):return s if ':' in s else 'minecraft:'+s
def data_path(kind,id,suffix):
 ns,path=ident(id).split(':',1);return f'assets/{ns}/{kind}/{path}{suffix}'
def put_json(path,data):entries[path]=(json.dumps(data,separators=(',',':'),ensure_ascii=False)+'\n').encode()
def source_resolved(id):
 result={};chain=[];seen=set();id=ident(id)
 while id in source_models:
  if id in seen:raise ValueError('cyclic source model '+id)
  seen.add(id);m=source_models[id];chain.append(m)
  if 'parent' not in m:break
  id=ident(m['parent'])
 for m in reversed(chain):
  result.update({k:copy.deepcopy(v) for k,v in m.items() if k not in ('textures','display')})
  result.setdefault('textures',{}).update(m.get('textures',{}))
  result.setdefault('display',{}).update(m.get('display',{}))
 return result
def texture_id(value,textures):
 seen=set()
 while value.startswith('#'):
  if value in seen:raise ValueError('cyclic texture '+value)
  seen.add(value);value=textures[value[1:]]
 return ident(value)
def copy_texture(value):
 if value.startswith('#'):return
 id=ident(value)
 if id.startswith('minecraft:'):return
 path=data_path('textures',id,'.png')
 if path not in jar.namelist():raise ValueError('missing source texture '+path)
 entries[path]=jar.read(path);copied_textures.add(id)
 if path+'.mcmeta' in jar.namelist():entries[path+'.mcmeta']=jar.read(path+'.mcmeta')
def image_for(value,textures):
 id=texture_id(value,textures)
 if id in pngs:return pngs[id]
 path=data_path('textures',id,'.png')
 if id.startswith('minecraft:'):raise ValueError('Vanilla material must stay a resource reference')
 im=Image.open(io.BytesIO(jar.read(path))).convert('RGBA')
 if path+'.mcmeta' in jar.namelist():
  meta=json.loads(jar.read(path+'.mcmeta')).get('animation',{})
  w=meta.get('width',im.width);h=meta.get('height',w)
  im=im.crop((0,0,w,h))
 pngs[texture_id(value,textures)]=np.array(im);return pngs[texture_id(value,textures)]
def read_obj(path,textures,flip):
 text=jar.read(path).decode();vertices=[];uv=[];faces=[];material=None;mats={}
 for line in text.splitlines():
  f=line.split()
  if not f:continue
  if f[0]=='mtllib':
   mtl=str(Path(path).parent/f[1]);current=None
   for row in jar.read(mtl).decode().splitlines():
    parts=row.split()
    if not parts:continue
    if parts[0]=='newmtl':current=parts[1]
    if parts[0]=='map_Kd':mats[current]=parts[-1]
  elif f[0]=='v':vertices.append([float(x)*16 for x in f[1:4]])
  elif f[0]=='vt':uv.append([float(f[1]),1-float(f[2]) if flip else float(f[2])])
  elif f[0]=='usemtl':material=f[1]
  elif f[0]=='f':
   ids=[x.split('/') for x in f[1:]]
   points=np.array([vertices[int(x[0])-1] for x in ids]);tex=np.array([uv[int(x[1])-1] for x in ids])
   faces.append((points,tex,mats[material]))
 return faces
# Basis columns for each native face's left-to-right / bottom-to-top / normal.
BASES=[('south',np.eye(3)),('east',np.array([[0,0,1],[0,1,0],[-1,0,0]],float)),('up',np.array([[1,0,0],[0,0,1],[0,-1,0]],float))]
def plane(points,uv,material,textures):
 u=points[1]-points[0];length=np.linalg.norm(u)
 if length<1e-7:return None
 u/=length;n=np.cross(u,points[2]-points[0]);norm=np.linalg.norm(n)
 if norm<1e-7:return None
 n/=norm;v=np.cross(n,u);basis=np.column_stack([u,v,n])
 xy=(points-points[0])@basis[:,:2];lo=xy.min(axis=0);hi=xy.max(axis=0);size=hi-lo
 if min(size)<1e-7:return None
 center=points[0]+basis[:,:2]@((lo+hi)/2)
 face,element=native_plane(center,size,basis)
 w,h=[max(2,min(512,math.ceil(float(s)*8))) for s in size]
 xx,yy=np.meshgrid(lo[0]+(np.arange(w)+0.5)/w*size[0],hi[1]-(np.arange(h)+0.5)/h*size[1])
 image=np.zeros((h,w,4),dtype=np.uint8);texture=image_for(material,textures);th,tw=texture.shape[:2]
 for i in range(1,len(points)-1):
  inds=[0,i,i+1];tri=xy[inds];tex=uv[inds]
  denom=(tri[1,1]-tri[2,1])*(tri[0,0]-tri[2,0])+(tri[2,0]-tri[1,0])*(tri[0,1]-tri[2,1])
  if abs(denom)<1e-10:continue
  b0=((tri[1,1]-tri[2,1])*(xx-tri[2,0])+(tri[2,0]-tri[1,0])*(yy-tri[2,1]))/denom
  b1=((tri[2,1]-tri[0,1])*(xx-tri[2,0])+(tri[0,0]-tri[2,0])*(yy-tri[2,1]))/denom;b2=1-b0-b1
  mask=(b0>=-1e-6)&(b1>=-1e-6)&(b2>=-1e-6)
  tx=b0*tex[0,0]+b1*tex[1,0]+b2*tex[2,0];ty=b0*tex[0,1]+b1*tex[1,1]+b2*tex[2,1]
  ix=np.clip(np.floor(tx*tw).astype(int),0,tw-1);iy=np.clip(np.floor(ty*th).astype(int),0,th-1)
  image[mask]=texture[iy[mask],ix[mask]]
 raw=io.BytesIO();Image.fromarray(image).save(raw,format='PNG',optimize=True);bytes_=raw.getvalue();key=hashlib.sha256(bytes_).hexdigest()[:24]
 path=f'assets/create/textures/block/horizon_obj/{key}.png';entries[path]=bytes_;resampled[key]=len(bytes_)
 element['faces']={face:{'uv':[0,0,16,16],'texture':'#obj_'+key}}
 return element,'obj_'+key,'create:block/horizon_obj/'+key

def ensure(id):
 id=ident(id)
 if id.startswith('minecraft:'):return
 if id in models:return
 if id in visiting:raise ValueError('cyclic model '+id)
 visiting.add(id)
 if id not in source_models:raise ValueError('missing source model '+id)
 model=copy.deepcopy(source_models[id]);loader=model.get('loader')
 if loader=='neoforge:obj':
  resolved=source_resolved(id);textures=resolved.get('textures',{});obj=data_path('',model['model'],'').replace('//','/')
  # OBJ resource location includes its models/ prefix already.
  ns,p=ident(model['model']).split(':',1);obj=f'assets/{ns}/{p}'
  elements=[];patches={};original_faces=read_obj(obj,textures,model.get('flip_v',False));faces=merge_rectangles(original_faces);skipped=0
  for points,uv,mat in faces:
   if texture_id(mat,textures).startswith('minecraft:'):
    faces_=convert_vanilla(points,uv,texture_id(mat,textures),report)
   else:
    converted_face=plane(points,uv,mat,textures)
    faces_=[] if converted_face is None else [converted_face]
   if not faces_:skipped+=1;continue
   for element,key,path in faces_:elements.append(element);patches[key]=path
  model.pop('loader',None);model.pop('model',None);model.pop('flip_v',None)
  model['elements']=elements;model.setdefault('textures',{}).update(patches)
  report['obj_conversions'][id]={'source_faces':len(original_faces),'merged_faces':len(faces),'elements':len(elements),'degenerate_faces_skipped':skipped}
 elif loader is not None:raise ValueError('Unsupported source loader '+str(loader)+' '+id)
 if 'parent' in model:ensure(model['parent'])
 for tex in model.get('textures',{}).values():
  if isinstance(tex,str) and not tex.startswith('#') and not tex.startswith('create:block/horizon_obj/'):
   copy_texture(tex)
 models[id]=model;visiting.remove(id)

def add_generated_model(id,model):
 models[id]=model
 for tex in model.get('textures',{}).values():
  if isinstance(tex,str) and not tex.startswith('#'):copy_texture(tex)

def assembly(parts,id):
 result={'textures':{},'elements':[],'ambientocclusion':False}
 for part,delta in parts:
  ensure(part);m=resolved_output(part);textures=m.get('textures',{})
  for element in copy.deepcopy(m.get('elements',[])):
   element.pop('name',None)
   for key in ['from','to']:element[key]=[round(x+d,7) for x,d in zip(element[key],delta)]
   if 'rotation' in element:element['rotation']['origin']=[round(x+d,7) for x,d in zip(element['rotation']['origin'],delta)]
   for face in element.get('faces',{}).values():
    if face.get('texture','').startswith('#'):
     texture=texture_id(face['texture'],textures);key='t'+hashlib.sha256(texture.encode()).hexdigest()[:12];result['textures'][key]=texture;face['texture']='#'+key
   result['elements'].append(element)
  if 'particle' in textures:result['textures']['particle']=texture_id(textures['particle'],textures)
 add_generated_model(id,result);return id

def resolved_output(id):
 result={};chain=[];id=ident(id);seen=set()
 while id in models:
  if id in seen:raise ValueError('output cycle')
  seen.add(id);m=models[id];chain.append(m)
  if 'parent' not in m:break
  id=ident(m['parent'])
 for m in reversed(chain):
  result.update({k:copy.deepcopy(v) for k,v in m.items() if k not in ('textures','display')})
  result.setdefault('textures',{}).update(m.get('textures',{}));result.setdefault('display',{}).update(m.get('display',{}))
 return result

blocks={n.rsplit('/',1)[1][:-5]:json.loads(d) for n,d in original.items() if '/horizon/block/' in n}
items={n.rsplit('/',1)[1][:-5]:json.loads(d) for n,d in original.items() if '/horizon/item/' in n}
for name in sorted(blocks|items):
 ensure('create:item/'+name)
 put_json(f'assets/create/items/{name}.json',{'model':{'type':'minecraft:model','model':'create:item/'+name}})
 report['item_ids'].append('create:'+name)
 print('Imported item model',name,flush=True)
# Whole static assemblies for placed machines whose real renderer adds parts.
assemblies={
 'mechanical_press':[('create:block/mechanical_press/block',[0,0,0]),('create:block/mechanical_press/head',[0,0,0])],
 'mechanical_mixer':[('create:block/mechanical_mixer/block',[0,0,0]),('create:block/mechanical_mixer/pole',[0,0,0]),('create:block/mechanical_mixer/head',[0,0,0])],
 'encased_fan':[('create:block/encased_fan/block',[0,0,0]),('create:block/encased_fan/propeller',[0,0,0])],
 'mechanical_drill':[('create:block/mechanical_drill/block',[0,0,0]),('create:block/mechanical_drill/head',[0,0,0])],
 'mechanical_saw':[('create:block/mechanical_saw/vertical',[0,0,0]),('create:block/mechanical_saw/blade_vertical_inactive',[0,0,0])],
 'hand_crank':[('create:block/hand_crank/block',[0,0,0]),('create:block/hand_crank/handle',[0,0,0])],
}
for name,decl in sorted(blocks.items()):
 model='create:item/'+name
 if name in assemblies:model=assembly(assemblies[name],'create:block/horizon_static/'+name)
 variants={'':{'model':model}}
 if decl['template']=='minecraft:basalt':
  variants={'axis=y':{'model':model},'axis=x':{'model':model,'x':90,'y':90},'axis=z':{'model':model,'x':90,'y':180}}
 put_json(f'assets/create/blockstates/{name}.json',{'variants':variants});report['static_block_ids'].append('create:'+name)
for id,model in sorted(models.items()):put_json(data_path('models',id,'.json'),model)
# Keep the actual localization for each registered item/block.
lang=json.loads(jar.read('assets/create/lang/en_us.json'))
put_json('assets/create/lang/en_us.json',{k:v for k,v in lang.items() if k in {'block.create.'+n for n in blocks}|{'item.create.'+n for n in items}})
entries['horizon-mod.toml']='''id = "create"
name = "Create: Horizon Content Preview"
version = "0.3.2"
description = "Original Create 6.0.10 textures and item models adapted from your supplied JAR. Static block previews only; machine gameplay is not included."
authors = ["Original assets: The Create Team", "Horizon format adaptation: Codex"]
api = "0.2"
side = "both"
capabilities = ["registries"]
'''.encode()
entries['NOTICE.md']=b'''Original Create assets supplied in the user's Create 6.0.10 JAR.\nCreate is copyright The Create Team / The Creators of Create.\nOriginal asset credits and model metadata are retained in the imported files.\nThis adapted package is for the user's private use, not a grant of redistribution rights.\nOriginal textures are retained where compatible; NeoForge OBJ models are converted to native static planes.\nNo Java code is included. The machine gameplay port remains unfinished.\n'''
entries['CREATE_LICENSE.md']=jar.read('LICENSE.md_create-1.21.1')
entries['INSTALL.txt']=b'''Create: Horizon Content Preview 0.3.2\nReplace the older create-horizon-content-preview.hmod in horizon-mods/.\nKeep only one content mod with id create enabled. Restart the game, then open your world.\nThis private-use adaptation imports original textures and item models from your supplied JAR.\nPlaced machines are static previews with approximate inherited collision boxes and some simplified orientation.\nIt does not add working machine logic, rotation, contraptions, trains or Ponder.\n'''
report.update({'models':len(models),'original_png_files':len(copied_textures),'obj_material_patch_sprites':len(resampled),'block_count':len(blocks),'separate_item_count':len(items)})
entries['ASSET_REPORT.json']=(json.dumps(report,indent=2)+'\n').encode()
with zipfile.ZipFile(OUT,'w') as z:
 for n,d in sorted(entries.items()):
  info=zipfile.ZipInfo(n,(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16;z.writestr(info,d)
(ROOT/'asset-work/report.json').write_text(json.dumps(report,indent=2)+'\n')
# Materialize only resulting mod data for format validation; no vanilla resources.
for n,d in entries.items():
 if n.startswith(('assets/','data/')):
  p=ROOT/'asset-work/result'/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(d)
print(json.dumps({'artifact':str(OUT),'bytes':OUT.stat().st_size,'models':len(models),'original_pngs':len(copied_textures),'OBJ':report['obj_conversions']},indent=2))
