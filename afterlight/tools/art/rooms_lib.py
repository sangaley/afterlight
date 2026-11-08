#!/usr/bin/env python3
"""Shared furnished-room primitive library for Afterlight module art.
Every module is a top-down room drawn at its true footprint aspect.
Import this from the per-category generator scripts."""
from PIL import Image, ImageDraw, ImageFilter
import os, math

SS=4              # supersample factor during drawing
FINAL=3           # final px per world-unit (matches approved medbay: 126wu -> 378px)
CELL=66           # world units per grid cell
PAD=60            # sizing pad (matches spawn_module: 60 + cells*66)

OUT=os.path.join(os.path.dirname(__file__),"room"); os.makedirs(OUT,exist_ok=True)

# ---- shared palette ----
EDGE=(20,24,30)
WALL=(60,70,82); WALL_HI=(120,134,150); WALL_LO=(40,48,58)
DECK=(92,102,114); DECK2=(84,94,106); GROUT=(58,66,78)
STEEL=(140,150,164); STEEL_L=(190,200,214); STEEL_D=(92,100,114)
DARK=(46,52,62)
# accents
ENERGY=(88,186,255); ENERGY_HI=(200,240,255); ENERGY_D=(40,90,150)
THRUST=(244,138,58); THRUST_HI=(255,214,150); THRUST_D=(150,60,20)
GREEN=(104,214,140); GREEN_HI=(190,245,205); GREEN_D=(40,120,70)
CYAN=(120,214,236); CYAN_D=(40,120,150)
AMBER=(224,176,86); AMBER_D=(150,104,40)
RED=(214,72,72); RED_D=(150,38,38); RED_HI=(255,224,224)
TEAL=(74,158,158); TEAL_L=(104,186,184)
CAUTION=(228,196,74)
SCREEN=(12,20,24); LED_G=(120,225,150); LED_A=(232,190,96); LED_R=(232,96,96)
BRASS=(196,168,96)

def A(c,a): return (c[0],c[1],c[2],a)
def mix(a,b,t): return tuple(int(a[i]*(1-t)+b[i]*t) for i in range(3))
def blur(im,r): return im.filter(ImageFilter.GaussianBlur(max(0.1,r*SS)))

class Room:
    """A footprint-aspect canvas with deck+walls already helpers."""
    def __init__(self, cw, ch):
        self.cw, self.ch = cw, ch
        self.WU_w = PAD + cw*CELL         # world-unit width
        self.WU_h = PAD + ch*CELL
        self.W = self.WU_w*FINAL*SS       # draw-space px
        self.H = self.WU_h*FINAL*SS
        self.img = Image.new("RGBA",(self.W,self.H),(0,0,0,0))
        self.d = ImageDraw.Draw(self.img)
        self.margin = 4*SS*FINAL/3.0       # keep margin ~ constant world units
    # unit helper: convert a 0..1 fraction of width/height
    def fx(self,t): return self.W*t
    def fy(self,t): return self.H*t
    def u(self,wu): return wu*FINAL*SS     # world-units -> draw px

    def box(self):
        m=self.u(4)
        return [m,m,self.W-m,self.H-m]

    def deck(self, tint=None, tile=CELL):
        d=self.d; x0,y0,x1,y1=self.box()
        base=DECK if tint is None else mix(DECK,tint,0.10)
        d.rounded_rectangle(self.box(),radius=self.u(6),fill=base+(255,))
        # tile grid at ~cell size
        stp=self.u(tile)
        gx=x0+stp
        while gx<x1-self.u(2):
            d.line([(gx,y0+self.u(3)),(gx,y1-self.u(3))],fill=GROUT+(150,),width=SS); gx+=stp
        gy=y0+stp
        while gy<y1-self.u(2):
            d.line([(x0+self.u(3),gy),(x1-self.u(3),gy)],fill=GROUT+(150,),width=SS); gy+=stp
        # faint checker for texture
        nx=max(1,int((x1-x0)/stp)); ny=max(1,int((y1-y0)/stp))
        for iy in range(ny+1):
            for ix in range(nx+1):
                if (ix+iy)%2==0:
                    cx0=x0+ix*stp; cy0=y0+iy*stp
                    d.rectangle([cx0,cy0,min(cx0+stp,x1),min(cy0+stp,y1)],fill=A(DECK2,50))

    def walls(self, accent=None):
        d=self.d; x0,y0,x1,y1=self.box()
        d.rounded_rectangle(self.box(),radius=self.u(8),outline=EDGE+(255,),width=self.u(6))
        d.rounded_rectangle([x0+self.u(5),y0+self.u(5),x1-self.u(5),y1-self.u(5)],radius=self.u(6),outline=WALL+(255,),width=self.u(4))
        d.rounded_rectangle([x0+self.u(7),y0+self.u(7),x1-self.u(7),y1-self.u(7)],radius=self.u(5),outline=WALL_HI+(60,),width=SS)
        if accent:
            d.rounded_rectangle([x0+self.u(7),y0+self.u(7),x1-self.u(7),y1-self.u(7)],radius=self.u(5),outline=A(accent,70),width=SS)
        for (bx,by) in [(x0+self.u(12),y0+self.u(12)),(x1-self.u(12),y0+self.u(12)),
                        (x0+self.u(12),y1-self.u(12)),(x1-self.u(12),y1-self.u(12))]:
            d.ellipse([bx-self.u(3),by-self.u(3),bx+self.u(3),by+self.u(3)],fill=WALL_LO+(255,))
            d.ellipse([bx-self.u(2),by-self.u(2),bx+self.u(1),by+self.u(1)],fill=STEEL_L+(200,))
        self.doors(accent)

    def doors(self, accent=None):
        """Cut a sliding hatch into the wall at each grid-cell edge, so
        doors on adjacent modules line up for future crew pathfinding.
        One door per column on top/bottom walls, per row on left/right."""
        d=self.d; x0,y0,x1,y1=self.box()
        band=self.u(9)                 # wall thickness (outer edge -> inner deck)
        dw=self.u(30)                  # door opening (~ crew width, < cell)
        col= accent if accent else STEEL_L
        cellw=(x1-x0)/self.cw; cellh=(y1-y0)/self.ch
        def hatch(cx,cy,horizontal):
            # threshold plate spanning the wall band, centred on (cx,cy) at the wall line
            if horizontal:   # door in a top/bottom wall -> opening runs along X
                bx0,bx1=cx-dw/2,cx+dw/2
                oy0,oy1=cy-band/2,cy+band/2
                d.rectangle([bx0,oy0,bx1,oy1],fill=DARK+(255,))              # recess (the gap)
                d.rectangle([bx0,oy0,bx1,oy1],outline=EDGE+(255,),width=SS)
                # retracted door leaves to each side (slides open)
                lw=self.u(5)
                d.rectangle([bx0-lw,oy0+SS,bx0,oy1-SS],fill=STEEL+(255,),outline=EDGE+(200,),width=SS)
                d.rectangle([bx1,oy0+SS,bx1+lw,oy1-SS],fill=STEEL+(255,),outline=EDGE+(200,),width=SS)
                # threshold tread lines + accent sill
                for t in (0.32,0.5,0.68):
                    yy=oy0+(oy1-oy0)*t
                    d.line([(bx0+SS,yy),(bx1-SS,yy)],fill=A(EDGE,120),width=SS)
                d.line([(bx0,cy),(bx1,cy)],fill=A(col,150),width=SS)
                # jamb bolts
                for jx in (bx0-lw/2,bx1+lw/2):
                    d.ellipse([jx-self.u(1.4),cy-self.u(1.4),jx+self.u(1.4),cy+self.u(1.4)],fill=WALL_LO+(255,))
            else:            # door in a left/right wall -> opening runs along Y
                by0,by1=cy-dw/2,cy+dw/2
                ox0,ox1=cx-band/2,cx+band/2
                d.rectangle([ox0,by0,ox1,by1],fill=DARK+(255,))
                d.rectangle([ox0,by0,ox1,by1],outline=EDGE+(255,),width=SS)
                lw=self.u(5)
                d.rectangle([ox0+SS,by0-lw,ox1-SS,by0],fill=STEEL+(255,),outline=EDGE+(200,),width=SS)
                d.rectangle([ox0+SS,by1,ox1-SS,by1+lw],fill=STEEL+(255,),outline=EDGE+(200,),width=SS)
                for t in (0.32,0.5,0.68):
                    xx=ox0+(ox1-ox0)*t
                    d.line([(xx,by0+SS),(xx,by1-SS)],fill=A(EDGE,120),width=SS)
                d.line([(cx,by0),(cx,by1)],fill=A(col,150),width=SS)
                for jy in (by0-lw/2,by1+lw/2):
                    d.ellipse([cx-self.u(1.4),jy-self.u(1.4),cx+self.u(1.4),jy+self.u(1.4)],fill=WALL_LO+(255,))
        for i in range(self.cw):
            cx=x0+(i+0.5)*cellw
            hatch(cx,y0+band/2-self.u(1),True)     # top
            hatch(cx,y1-band/2+self.u(1),True)     # bottom
        for j in range(self.ch):
            cy=y0+(j+0.5)*cellh
            hatch(x0+band/2-self.u(1),cy,False)    # left
            hatch(x1-band/2+self.u(1),cy,False)    # right

    def shadow(self, box, grow=1.4, dy=1.2, alpha=95):
        sh=Image.new("RGBA",self.img.size,(0,0,0,0))
        g=self.u(grow); dd=self.u(dy)
        ImageDraw.Draw(sh).rounded_rectangle([box[0]-g,box[1]-g+dd,box[2]+g,box[3]+g+dd],
                                             radius=self.u(4),fill=(0,0,0,alpha))
        self.img.alpha_composite(blur(sh,2)); self.d=ImageDraw.Draw(self.img)

    def panel(self, box, fill=STEEL, outline=EDGE, rad=3, hi=True, shadow=True):
        if shadow: self.shadow(box)
        self.d.rounded_rectangle(box,radius=self.u(rad),fill=fill+(255,),outline=outline+(255,),width=SS*2)
        if hi:
            self.d.rounded_rectangle([box[0]+SS,box[1]+SS,box[2]-SS,box[1]+self.u(2)],radius=SS,fill=A(STEEL_L,90))

    def screen(self, box, col=ENERGY, waveform=True, grid=False):
        d=self.d
        d.rounded_rectangle(box,radius=self.u(2),fill=SCREEN+(255,),outline=EDGE+(255,),width=SS)
        x0,y0,x1,y1=box; span=x1-x0; yb=(y0+y1)/2
        if grid:
            for gx in range(int(x0),int(x1),int(span/5)):
                d.line([(gx,y0),(gx,y1)],fill=A(col,40),width=SS)
        if waveform:
            xs=[0.10,0.28,0.34,0.40,0.5,0.63,0.80,0.94]; ys=[0,0,-0.85,0.7,0,0,0,0]
            pts=[(x0+self.u(1),yb)]
            for t,yy in zip(xs,ys): pts.append((x0+span*t, yb+(y1-y0)*0.40*yy))
            pts.append((x1-self.u(1),yb))
            d.line(pts,fill=col+(255,),width=SS)
        # bezel LEDs
        d.ellipse([x0+self.u(1),y1-self.u(2.5),x0+self.u(2.5),y1-self.u(1)],fill=LED_G+(255,))

    def glow_core(self, cx, cy, R, col=ENERGY, colhi=ENERGY_HI, rings=True):
        d=self.d
        d.ellipse([cx-R,cy-R,cx+R,cy+R],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS*2)
        if rings:
            d.ellipse([cx-R+self.u(3),cy-R+self.u(3),cx+R-self.u(3),cy+R-self.u(3)],outline=A(STEEL_L,70),width=SS)
        g=Image.new("RGBA",self.img.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
        N=28
        for i in range(N,0,-1):
            t=i/N; rr=R*0.78*t; c=mix(col,colhi,t)
            gd.ellipse([cx-rr,cy-rr,cx+rr,cy+rr],fill=c+(int(40+200*(1-t)),))
        self.img.alpha_composite(blur(g,2.2)); self.d=ImageDraw.Draw(self.img)
        self.d.ellipse([cx-self.u(4),cy-self.u(4),cx+self.u(4),cy+self.u(4)],fill=colhi+(255,))

    def hazard_edge(self, top=True, bottom=True, col=CAUTION):
        d=self.d; x0,y0,x1,y1=self.box()
        step=self.u(18)
        edges=[]
        if top: edges.append(y0+self.u(9))
        if bottom: edges.append(y1-self.u(13))
        for ey in edges:
            sx=x0+self.u(12)
            while sx<x1-self.u(20):
                d.polygon([(sx,ey),(sx+self.u(9),ey),(sx+self.u(4.5),ey+self.u(5))],fill=A(col,150)); sx+=step

    def pipe(self, x0,y0,x1,y1, w=6, col=STEEL_L):
        self.d.line([(x0,y0),(x1,y1)],fill=col+(255,),width=self.u(w))
        self.d.line([(x0,y0),(x1,y1)],fill=A(EDGE,120),width=SS)

    def bolts(self, box, n=4):
        x0,y0,x1,y1=box
        pts=[(x0+self.u(3),y0+self.u(3)),(x1-self.u(3),y0+self.u(3)),
             (x0+self.u(3),y1-self.u(3)),(x1-self.u(3),y1-self.u(3))]
        for (bx,by) in pts:
            self.d.ellipse([bx-self.u(1.5),by-self.u(1.5),bx+self.u(1.5),by+self.u(1.5)],fill=STEEL_D+(255,))

    def save(self, name):
        fw=self.WU_w*FINAL; fh=self.WU_h*FINAL
        out=self.img.resize((int(fw),int(fh)),Image.LANCZOS)
        out.save(f"{OUT}/{name}")
        return out.size
