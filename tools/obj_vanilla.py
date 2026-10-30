"""Keep vanilla material resource references; never package Mojang textures."""
import hashlib,warnings
from native_plane import native_plane
import numpy as np
from scipy.spatial.transform import Rotation
BASES=[('south',np.eye(3)),('east',np.array([[0,0,1],[0,1,0],[-1,0,0]],float)),('up',np.array([[1,0,0],[0,0,1],[0,-1,0]],float))]
def convert(points,uv,material,report):
    aff=np.linalg.lstsq(np.column_stack([uv,np.ones(len(uv))]),points,rcond=None)[0]
    u=aff[0];u/=np.linalg.norm(u);v=-aff[1];v-=u*np.dot(u,v)
    if np.linalg.norm(v)<1e-7:return []
    v/=np.linalg.norm(v)
    normal=np.cross(points[1]-points[0],points[2]-points[0]);normal/=np.linalg.norm(normal)
    if np.dot(np.cross(u,v),normal)<0:u=-u
    n=np.cross(u,v);basis=np.column_stack([u,v,n]);origin=points[0]
    xy=(points-origin)@basis[:,:2];lo=xy.min(0);hi=xy.max(0)
    uvmap=np.linalg.lstsq(np.column_stack([xy,np.ones(len(xy))]),uv,rcond=None)[0]
    rectangle=len(points)==4 and all(any(np.linalg.norm(p-q)<1e-4 for p in xy) for q in [[lo[0],lo[1]],[lo[0],hi[1]],[hi[0],lo[1]],[hi[0],hi[1]]])
    rects=[]
    if rectangle:rects=[(*lo,*hi)]
    else:
        # Conservative geometric strips retain the exact vanilla material key.
        # No sampled vanilla pixels are included in this private asset package.
        report['vanilla_triangle_strip_faces']=report.get('vanilla_triangle_strip_faces',0)+1
        def interval(y):
            xs=[]
            for p,q in zip(xy,np.roll(xy,-1,axis=0)):
                if min(p[1],q[1])-1e-7<=y<=max(p[1],q[1])+1e-7 and abs(p[1]-q[1])>1e-9:
                    t=(y-p[1])/(q[1]-p[1]);xs.append(p[0]+t*(q[0]-p[0]))
            return (min(xs),max(xs)) if len(xs)>=2 else None
        for i in range(16):
            a=lo[1]+(hi[1]-lo[1])*i/16;b=lo[1]+(hi[1]-lo[1])*(i+1)/16
            one=interval(a+1e-7);two=interval(b-1e-7)
            if one and two:
                x=max(one[0],two[0]);X=min(one[1],two[1])
                if X-x>1e-7:rects.append((x,a,X,b))
    result=[];key='vanilla_'+hashlib.sha256(material.encode()).hexdigest()[:14]
    for x,y,X,Y in rects:
        size=np.array([X-x,Y-y]);center=origin+basis[:,:2]@np.array([(x+X)/2,(y+Y)/2])
        if min(size)<1e-5:continue
        face,element=native_plane(center,size,basis)
        targets=np.column_stack([[[x,Y],[x,y],[X,y],[X,Y]],np.ones(4)])@uvmap
        mi=targets.min(0);ma=targets.max(0);best=None
        for reverse_u in (False,True):
            for reverse_v in (False,True):
                U0,U1=(ma[0],mi[0]) if reverse_u else (mi[0],ma[0]);V0,V1=(ma[1],mi[1]) if reverse_v else (mi[1],ma[1])
                raw=np.array([[U0,V0],[U0,V1],[U1,V1],[U1,V0]])
                for rot in range(4):
                    actual=np.roll(raw,-rot,axis=0);err=np.max(np.abs(actual-targets))
                    if best is None or err<best[0]:best=(err,[U0*16,V0*16,U1*16,V1*16],rot*90)
        err,bounds,rot=best
        if err>0.005:report['vanilla_uv_approximations']=report.get('vanilla_uv_approximations',0)+1
        element['faces']={face:{'uv':np.round(bounds,7).tolist(),'rotation':rot,'texture':'#'+key}}
        result.append((element,key,material))
    return result
