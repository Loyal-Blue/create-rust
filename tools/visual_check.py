"""Internal model QA from the native Horizon baker, without game resources."""
from pathlib import Path
import json,numpy as np
from PIL import Image,ImageDraw
from scipy.spatial.transform import Rotation
root=Path(__file__).resolve().parents[1]
models=json.loads((root/'asset-work/baked-preview.json').read_text())
size=280;out=Image.new('RGB',(size*3,size*((len(models)+2)//3)),(28,33,40));draw=ImageDraw.Draw(out);textures={}
view=Rotation.from_euler('yx',[-35,25],degrees=True).as_matrix()
for index,(name,faces) in enumerate(models.items()):
 tile=np.full((size,size,3),[28,33,40],dtype=np.uint8);depth=np.full((size,size),-np.inf)
 pts=np.array([p for f in faces for p in f['positions']]);center=(pts.min(0)+pts.max(0))/2
 transformed=(pts-center)@view.T;extent=max(np.ptp(transformed[:,0]),np.ptp(transformed[:,1]));scale=200/max(extent,1e-6)
 for face in faces:
  p=(np.array(face['positions'])-center)@view.T
  normal=np.cross(p[1]-p[0],p[2]-p[0]);length=np.linalg.norm(normal)
  if length<1e-8:continue
  normal/=length
  if normal[2]<-1e-5:continue
  screen=np.column_stack([p[:,0]*scale+size/2,-p[:,1]*scale+size/2+6,p[:,2]])
  uv=np.array(face['uv']);sprite=face['sprite']
  if sprite not in textures:
   ns,path=sprite.split(':',1);file=root/f'asset-work/result/assets/{ns}/textures/{path}.png'
   if not file.exists() and ns=='minecraft':
    # Neutral debug wood color only in this internal render. The deliverable
    # references the actual vanilla texture provided by the user's game.
    im=Image.new('RGBA',(16,16),(120,94,63,255))
   else:im=Image.open(file).convert('RGBA')
   if Path(str(file)+'.mcmeta').exists():im=im.crop((0,0,im.width,im.width))
   textures[sprite]=np.array(im)
  texture=textures[sprite];th,tw=texture.shape[:2]
  shade=0.55+0.45*max(0,float(np.dot(normal,np.array([-.4,.7,.8])/np.linalg.norm([-.4,.7,.8]))))
  for ids in [[0,1,2],[0,2,3]]:
   a,b,c=screen[ids];coords=uv[ids];den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
   if abs(den)<1e-8:continue
   x0=max(0,int(np.floor(min(a[0],b[0],c[0]))));x1=min(size-1,int(np.ceil(max(a[0],b[0],c[0]))));y0=max(0,int(np.floor(min(a[1],b[1],c[1]))));y1=min(size-1,int(np.ceil(max(a[1],b[1],c[1]))))
   if x1<x0 or y1<y0:continue
   xx,yy=np.meshgrid(np.arange(x0,x1+1)+.5,np.arange(y0,y1+1)+.5)
   w0=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den;w1=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den;w2=1-w0-w1
   z=w0*a[2]+w1*b[2]+w2*c[2];mask=(w0>=-1e-5)&(w1>=-1e-5)&(w2>=-1e-5)&(z>depth[y0:y1+1,x0:x1+1])
   u=w0*coords[0,0]+w1*coords[1,0]+w2*coords[2,0];v=w0*coords[0,1]+w1*coords[1,1]+w2*coords[2,1]
   ix=np.clip((u*tw).astype(int),0,tw-1);iy=np.clip((v*th).astype(int),0,th-1);color=texture[iy,ix];mask&=color[:,:,3]>127
   tile[y0:y1+1,x0:x1+1][mask]=(color[:,:,:3][mask]*shade).astype(np.uint8);depth[y0:y1+1,x0:x1+1][mask]=z[mask]
 x=(index%3)*size;y=(index//3)*size;out.paste(Image.fromarray(tile),(x,y));draw.text((x+14,y+12),name.split('/')[-1],fill=(235,239,244))
out.save(root/'asset-work/visual-check.png')
