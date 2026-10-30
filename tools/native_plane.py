"""Encode a mesh plane within native element bounds using a free rotation pivot."""
import warnings
import numpy as np
from scipy.spatial.transform import Rotation
BASES=[('south',np.eye(3)),('east',np.array([[0,0,1],[0,1,0],[-1,0,0]],float)),('up',np.array([[1,0,0],[0,0,1],[0,-1,0]],float)),('north',np.diag([-1,1,-1])),('west',np.array([[0,0,-1],[0,1,0],[1,0,0]],float)),('down',np.array([[1,0,0],[0,0,-1],[0,1,0]],float))]
def native_plane(center,size,basis):
    for face,b in BASES:
        radius=(np.abs(b[:,0])*size[0]+np.abs(b[:,1])*size[1])/2
        lower=-16+radius;upper=32-radius
        if (lower>upper).any():continue
        matrix=basis@b.T
        rotation=Rotation.from_matrix(matrix);vector=rotation.as_rotvec();angle=np.linalg.norm(vector)
        if angle<1e-7:
            if (center<lower-1e-6).any() or (center>upper+1e-6).any():continue
            base=center.copy();pivot=center.copy()
        else:
            axis=vector/angle;base=np.clip(center,lower,upper)
            residual=float(np.dot(axis,center-base))
            for k in sorted(range(3),key=lambda k:-abs(axis[k])):
                if abs(axis[k])<1e-9:continue
                value=np.clip(base[k]+residual/axis[k],lower[k],upper[k]);residual-=axis[k]*(value-base[k]);base[k]=value
            if abs(residual)>1e-5:continue
            pivot=np.linalg.lstsq(np.eye(3)-matrix,center-matrix@base,rcond=None)[0]
            if np.max(np.abs(pivot+matrix@(base-pivot)-center))>1e-5:continue
        with warnings.catch_warnings():
            warnings.simplefilter('ignore');angles=rotation.as_euler('xyz',degrees=True)
        return face,{'from':np.clip(base-radius,-16,32).round(7).tolist(),'to':np.clip(base+radius,-16,32).round(7).tolist(),'rotation':{'origin':pivot.round(7).tolist(),**dict(zip('xyz',angles.round(7).tolist()))}}
    raise ValueError('Unable to encode plane center='+str(center)+' size='+str(size))
