"""Recover rectangular mesh quads from adjacent exported OBJ triangles."""
from collections import defaultdict
import numpy as np
def merge_rectangles(faces):
    edge_map=defaultdict(list)
    def key(p,t):return tuple(np.round(np.r_[p,t],6))
    for i,(p,t,m) in enumerate(faces):
        if len(p)!=3:continue
        keys=[key(a,b) for a,b in zip(p,t)]
        for a,b in zip(keys,keys[1:]+keys[:1]):edge_map[(m,tuple(sorted((a,b))))].append(i)
    used=set();out=[]
    for i,(points,uv,material) in enumerate(faces):
        if i in used:continue
        paired=False
        if len(points)==3:
            keys=[key(a,b) for a,b in zip(points,uv)]
            candidates={j for a,b in zip(keys,keys[1:]+keys[:1]) for j in edge_map[(material,tuple(sorted((a,b))))] if j!=i and j not in used}
            for j in candidates:
                p2,t2,m2=faces[j];all_={key(p,t):(p,t) for p,t in zip(np.vstack([points,p2]),np.vstack([uv,t2]))}
                if len(all_)!=4:continue
                ps=np.array([v[0] for v in all_.values()]);ts=np.array([v[1] for v in all_.values()])
                n=np.cross(points[1]-points[0],points[2]-points[0]);length=np.linalg.norm(n)
                if length<1e-8:continue
                n/=length
                n2=np.cross(p2[1]-p2[0],p2[2]-p2[0]);len2=np.linalg.norm(n2)
                if len2<1e-8 or np.dot(n,n2/len2)<0.9999:continue
                c=ps.mean(0);a=ps[0]-c;a/=np.linalg.norm(a);b=np.cross(n,a)
                angle=np.arctan2((ps-c)@b,(ps-c)@a);order=np.argsort(angle);ps=ps[order];ts=ts[order]
                edges=np.roll(ps,-1,axis=0)-ps;lens=np.linalg.norm(edges,axis=1)
                if min(lens)<1e-6:continue
                if max(abs(np.dot(edges[k],edges[(k+1)%4])/(lens[k]*lens[(k+1)%4])) for k in range(4))>0.001:continue
                if abs(lens[0]-lens[2])+abs(lens[1]-lens[3])>1e-3:continue
                out.append((ps,ts,material));used.add(j);paired=True;break
        if not paired:out.append((points,uv,material))
        used.add(i)
    return out
