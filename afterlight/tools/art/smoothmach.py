#!/usr/bin/env python3
"""SMOOTH machinery/fixture set — the look the user prefers, with the layout
they asked for: weapons/sensors/command/lights/power/propulsion/utility are
apparatus mounted on the bare hull (transparent bg, NO deck/walls/doors, not a
full block). Crew rooms + cargo keep their furnished smooth versions.
Reuses rooms_lib smooth primitives (supersampled + LANCZOS)."""
from rooms_lib import *   # Room, palette, A, mix, blur, SS, FINAL, CELL, PAD, OUT dir
import math

MDIR=os.path.join(os.path.dirname(__file__),"smooth_mach"); os.makedirs(MDIR,exist_ok=True)

# a few extra accent shades
COP=(184,120,60); COPL=(228,170,96); COPD=(120,74,38)
BRASS=(198,162,80); BRASSL=(234,208,128)
ORG=THRUST; ORGL=(255,214,150); ORGW=(255,246,224)

GRAY=(90,94,102)   # plain neutral backing — NOT the tiled/blue deck used for rooms

def backplate(r):
    """A plain gray backing panel filling the footprint so machinery isn't
    floating on empty space. No tile grid, no walls, no door hatches — this is
    a backing plate, not a floor. Distinct neutral gray vs the rooms' deck."""
    d=r.d; b=[U(r,4),U(r,4),U(r,r.WU_w-4),U(r,r.WU_h-4)]
    d.rounded_rectangle(b,radius=U(r,7),fill=GRAY+(255,),outline=EDGE+(255,),width=SS*2)
    # subtle bevel: light top, dark bottom
    d.line([(b[0]+U(r,5),b[1]+SS*2),(b[2]-U(r,5),b[1]+SS*2)],fill=A(mix(GRAY,(255,255,255),0.28),160),width=SS)
    d.line([(b[0]+U(r,5),b[3]-SS*2),(b[2]-U(r,5),b[3]-SS*2)],fill=A(EDGE,150),width=SS)
    # faint inner outline for a machined edge
    d.rounded_rectangle([b[0]+U(r,2.5),b[1]+U(r,2.5),b[2]-U(r,2.5),b[3]-U(r,2.5)],radius=U(r,5),outline=A(mix(GRAY,(255,255,255),0.12),60),width=SS)
    # corner bolts
    for (rx,ry) in [(9,9),(r.WU_w-9,9),(9,r.WU_h-9),(r.WU_w-9,r.WU_h-9)]:
        d.ellipse([U(r,rx-1.8),U(r,ry-1.8),U(r,rx+1.8),U(r,ry+1.8)],fill=mix(GRAY,EDGE,0.5)+(255,))
        d.ellipse([U(r,rx-1.1),U(r,ry-1.1),U(r,rx+0.3),U(r,ry+0.3)],fill=A(mix(GRAY,(255,255,255),0.4),220))

def R(cw,ch):
    r=Room(cw,ch); r.img=Image.new("RGBA",(r.W,r.H),(0,0,0,0)); r.d=ImageDraw.Draw(r.img)
    backplate(r)
    return r
def U(r,v): return r.u(v)          # world-units -> draw px
def cxw(r): return r.WU_w/2
def save(r,name):
    fw=r.WU_w*FINAL; fh=r.WU_h*FINAL
    r.img.resize((int(fw),int(fh)),Image.LANCZOS).save(f"{MDIR}/{name}"); print("·",name)
def layer(r): g=Image.new("RGBA",r.img.size,(0,0,0,0)); return g,ImageDraw.Draw(g)
def compose(r,g): r.img.alpha_composite(g); r.d=ImageDraw.Draw(r.img)

# ---------- smooth machinery primitives (coords in WORLD UNITS) ----------
def plate(r, x0,y0,x1,y1, rad=8, base=STEEL, hazard=False, rivets=True):
    d=r.d; b=[U(r,x0),U(r,y0),U(r,x1),U(r,y1)]
    # drop shadow
    r.shadow(b, grow=1.2, dy=1.4, alpha=90)
    d.rounded_rectangle(b,radius=U(r,rad),fill=base+(255,),outline=EDGE+(255,),width=SS*2)
    # bevel
    d.line([(b[0]+U(r,rad*0.4),b[1]+SS*2),(b[2]-U(r,rad*0.4),b[1]+SS*2)],fill=A(STEEL_L,150),width=SS)
    d.line([(b[0]+U(r,rad*0.4),b[3]-SS*2),(b[2]-U(r,rad*0.4),b[3]-SS*2)],fill=A(EDGE,150),width=SS)
    # panel seam
    my=(y0+y1)/2
    d.line([(b[0]+SS*2,U(r,my)),(b[2]-SS*2,U(r,my))],fill=A(EDGE,120),width=SS)
    d.line([(b[0]+SS*2,U(r,my)+SS),(b[2]-SS*2,U(r,my)+SS)],fill=A(STEEL_L,50),width=SS)
    if rivets:
        for (rx,ry) in [(x0+4,y0+4),(x1-4,y0+4),(x0+4,y1-4),(x1-4,y1-4)]:
            d.ellipse([U(r,rx-1.6),U(r,ry-1.6),U(r,rx+1.6),U(r,ry+1.6)],fill=STEEL_D+(255,))
            d.ellipse([U(r,rx-1),U(r,ry-1),U(r,rx+0.4),U(r,ry+0.4)],fill=STEEL_L+(230,))
    if hazard:
        hz=[U(r,x0+3),U(r,y1-3.5),U(r,x1-3),U(r,y1-1.5)]
        d.rectangle(hz,fill=(210,180,70,255))
        # chevrons
        step=U(r,6)
        sx=hz[0]-step
        while sx<hz[2]:
            d.polygon([(sx,hz[3]),(sx+step*0.5,hz[3]),(sx+step*0.5+(hz[3]-hz[1]),hz[1]),(sx+(hz[3]-hz[1]),hz[1])],fill=(34,38,44,255))
            sx+=step
        d.rectangle(hz,outline=EDGE+(255,),width=SS)

def turret(r, cx, cy, rad, gear=True):
    d=r.d
    d.ellipse([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    if gear:
        for a in range(0,360,18):
            nx=cx+math.cos(math.radians(a))*(rad-0.5); ny=cy+math.sin(math.radians(a))*(rad-0.5)
            d.ellipse([U(r,nx-0.7),U(r,ny-0.7),U(r,nx+0.7),U(r,ny+0.7)],fill=EDGE+(200,))
    d.ellipse([U(r,cx-rad+4),U(r,cy-rad+4),U(r,cx+rad-4),U(r,cy+rad-4)],fill=STEEL_D+(255,),outline=A(STEEL_L,90),width=SS)
    d.arc([U(r,cx-rad+4),U(r,cy-rad+4),U(r,cx+rad-4),U(r,cy+rad-4)],20,160,fill=A(STEEL_L,150),width=SS)

def barrel(r, cx, top, bottom, w, rail=False, energy=ENERGY):
    d=r.d
    d.rounded_rectangle([U(r,cx-w/2),U(r,top),U(r,cx+w/2),U(r,bottom)],radius=U(r,w*0.3),
                        fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
    d.line([(U(r,cx-w/2+0.6),U(r,top+1)),(U(r,cx-w/2+0.6),U(r,bottom-1))],fill=A((235,242,250),200),width=SS)
    d.line([(U(r,cx+w/2-0.6),U(r,top+1)),(U(r,cx+w/2-0.6),U(r,bottom-1))],fill=A(EDGE,150),width=SS)
    # muzzle
    d.rounded_rectangle([U(r,cx-w*0.8),U(r,top),U(r,cx+w*0.8),U(r,top+3)],radius=U(r,1),fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    if rail:
        g,gd=layer(r); n=int((bottom-top)/6)
        for i in range(n):
            yy=top+3+i*6
            gd.rounded_rectangle([U(r,cx-w*0.35),U(r,yy),U(r,cx+w*0.35),U(r,yy+2.5)],radius=U(r,0.6),fill=A(energy,230))
            gd.line([(U(r,cx-w*0.3),U(r,yy+1.2)),(U(r,cx+w*0.3),U(r,yy+1.2))],fill=A(mix(energy,(255,255,255),0.5),255),width=SS)
        compose(r,blur(g,0.4))

def coilbank(r, x, y0, y1, w):
    d=r.d
    d.rounded_rectangle([U(r,x),U(r,y0),U(r,x+w),U(r,y1)],radius=U(r,1.5),fill=mix(STEEL_D,EDGE,0.3)+(255,),outline=EDGE+(255,),width=SS)
    yy=y0+1.5; k=0
    while yy<y1-1.5:
        d.rounded_rectangle([U(r,x+1),U(r,yy),U(r,x+w-1),U(r,yy+2)],radius=U(r,0.6),fill=(COPL if k%2==0 else COP)+(255,))
        d.line([(U(r,x+1.4),U(r,yy+0.5)),(U(r,x+w-1.4),U(r,yy+0.5))],fill=A(COPL,200),width=SS)
        yy+=3.2; k+=1

def cylinder(r, cx, y0, y1, w, fill, fluid=None):
    d=r.d; x0,x1=cx-w/2,cx+w/2
    d.rounded_rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],radius=U(r,w/2),fill=fill+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(U(r,x0+1.2),U(r,y0+2)),(U(r,x0+1.2),U(r,y1-2))],fill=A(STEEL_L,180),width=SS)
    d.line([(U(r,x1-1.2),U(r,y0+2)),(U(r,x1-1.2),U(r,y1-2))],fill=A(EDGE,150),width=SS)
    if fluid:
        fy0=y0+(y1-y0)*0.34
        d.rounded_rectangle([U(r,x0+2),U(r,fy0),U(r,x1-2),U(r,y1-2)],radius=U(r,w*0.3),fill=A(fluid,170))
        d.line([(U(r,x0+2),U(r,fy0)),(U(r,x1-2),U(r,fy0))],fill=A(mix(fluid,(255,255,255),0.4),220),width=SS)
    for by in [y0+(y1-y0)*t for t in (0.25,0.5,0.75)]:
        d.line([(U(r,x0),U(r,by)),(U(r,x1),U(r,by))],fill=A(EDGE,110),width=SS)

def scr(r, box, col=CYAN, wave=True, grid=True):
    b=[U(r,box[0]),U(r,box[1]),U(r,box[2]),U(r,box[3])]
    r.d.rounded_rectangle(b,radius=U(r,1.5),fill=(10,24,28,255),outline=EDGE+(255,),width=SS)
    x0,y0,x1,y1=b; yb=(y0+y1)/2; sp=x1-x0
    if grid:
        gy=y0
        while gy<y1: r.d.line([(x0,gy),(x1,gy)],fill=A(col,35),width=SS); gy+=U(r,3)
    if wave:
        r.d.line([(x0+U(r,1),yb),(x0+sp*0.3,yb-sp*0.10),(x0+sp*0.5,yb+sp*0.10),(x0+sp*0.7,yb-sp*0.04),(x1-U(r,1),yb)],fill=col+(255,),width=SS)
    r.d.ellipse([x0+U(r,1),y1-U(r,2.5),x0+U(r,2.5),y1-U(r,1)],fill=(120,225,150,255))

def chair(r, cx, cy, big=False):
    d=r.d; w=6.5 if big else 5
    d.rounded_rectangle([U(r,cx-w),U(r,cy-1),U(r,cx+w),U(r,cy+8)],radius=U(r,2.5),fill=(52,66,96,255),outline=EDGE+(255,),width=SS)
    d.rounded_rectangle([U(r,cx-w+1.5),U(r,cy+1),U(r,cx+w-1.5),U(r,cy+6)],radius=U(r,2),fill=(74,92,126,255))
    d.rounded_rectangle([U(r,cx-w+1),U(r,cy-3.5),U(r,cx+w-1),U(r,cy+0.5)],radius=U(r,1.5),fill=(86,104,140,255),outline=EDGE+(200,),width=SS)

def softglow(r, cx, cy, rad, col, colhi):
    g,gd=layer(r)
    N=22
    for i in range(N,0,-1):
        t=i/N; rr=rad*t
        c=mix(col,colhi,1-t)
        gd.ellipse([U(r,cx-rr),U(r,cy-rr),U(r,cx+rr),U(r,cy+rr)],fill=c+(int(30+90*(t)),))
    gd.ellipse([U(r,cx-rad*0.2),U(r,cy-rad*0.2),U(r,cx+rad*0.2),U(r,cy+rad*0.2)],fill=colhi+(255,))
    compose(r,blur(g,1.4))

def vent(r, x0,y0,x1,y1):
    d=r.d
    d.rounded_rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],radius=U(r,0.8),fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(200,),width=SS)
    gy=y0+1.2
    while gy<y1-0.6:
        d.line([(U(r,x0+1),U(r,gy)),(U(r,x1-1),U(r,gy))],fill=A(STEEL,150),width=SS); gy+=U(r,1.6)/U(r,1)*1.4+0.6

def cable(r, pts, col=STEEL_D):
    p=[(U(r,x),U(r,y)) for (x,y) in pts]
    r.d.line(p,fill=EDGE+(255,),width=SS*3); r.d.line(p,fill=col+(255,),width=SS)

def flange_pipes(r):
    d=r.d; W,Hh=r.WU_w,r.WU_h; cx,cy=W/2,Hh/2
    for (fx,fy) in [(14,14),(W-14,14),(14,Hh-14),(W-14,Hh-14)]:
        d.rounded_rectangle([U(r,fx-5),U(r,fy-5),U(r,fx+5),U(r,fy+5)],radius=U(r,1.5),fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS)
    for (a,b) in [((18,16),(cx,16)),((W-18,16),(cx,16)),((16,18),(16,cy)),((W-16,18),(W-16,cy))]:
        r.pipe(U(r,a[0]),U(r,a[1]),U(r,b[0]),U(r,b[1]),w=4,col=STEEL)

# ==================================================================== POWER
def small_reactor():
    r=R(1,1); W=r.WU_w; cx=cy=W/2
    flange_pipes(r)
    d=r.d
    d.ellipse([U(r,cx-38),U(r,cy-38),U(r,cx+38),U(r,cy+38)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([U(r,cx-38),U(r,cy-38),U(r,cx+38),U(r,cy+38)],20,160,fill=A(STEEL_L,150),width=SS)
    for a0 in range(0,360,45):
        d.arc([U(r,cx-30),U(r,cy-30),U(r,cx+30),U(r,cy+30)],a0+6,a0+38,fill=COP+(255,),width=SS*4)
        d.arc([U(r,cx-30),U(r,cy-30),U(r,cx+30),U(r,cy+30)],a0+6,a0+38,fill=A(COPL,220),width=SS)
    d.ellipse([U(r,cx-23),U(r,cy-23),U(r,cx+23),U(r,cy+23)],fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    softglow(r,cx,cy,20,ENERGY,ENERGY_HI)
    d=r.d
    for a in range(0,360,60):
        d.line([(U(r,cx),U(r,cy)),(U(r,cx+math.cos(math.radians(a))*15),U(r,cy+math.sin(math.radians(a))*15))],fill=A((44,80,130),220),width=SS)
    plate(r,cx-17,W-24,cx+17,W-8,rad=3,base=STEEL_D,rivets=False)
    scr(r,[cx-14,W-21,cx-1,W-11],col=ENERGY,grid=False)
    d.ellipse([U(r,cx+3),U(r,W-19),U(r,cx+6),U(r,W-16)],fill=(120,225,150,255))
    d.ellipse([U(r,cx+8),U(r,W-19),U(r,cx+11),U(r,W-16)],fill=(232,190,96,255))
    save(r,"small_reactor.png")

def large_reactor(cw,ch,name):
    r=R(cw,ch); W,Hh=r.WU_w,r.WU_h; cx,cy=W/2,Hh/2
    d=r.d
    for fy in (Hh*0.26,Hh*0.74):
        r.pipe(U(r,14),U(r,fy),U(r,W-14),U(r,fy),w=5,col=STEEL_L)
    rad=min(W,Hh)/2-26
    d.ellipse([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],20,160,fill=A(STEEL_L,150),width=SS)
    for a0 in range(0,360,40):
        d.arc([U(r,cx-rad+6),U(r,cy-rad+6),U(r,cx+rad-6),U(r,cy+rad-6)],a0+5,a0+33,fill=COP+(255,),width=SS*4)
    d.ellipse([U(r,cx-rad+16),U(r,cy-rad+16),U(r,cx+rad-16),U(r,cy+rad-16)],fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    softglow(r,cx,cy,rad-16,ENERGY,ENERGY_HI)
    if cw>=3:
        for (fx,fy) in [(0.24,0.26),(0.76,0.26),(0.24,0.74),(0.76,0.74)]:
            softglow(r,W*fx,Hh*fy,10,ENERGY,ENERGY_HI)
    d=r.d
    for px,ac in ((16,ENERGY),(W-16-28,AMBER)):
        plate(r,px,cy-14,px+28,cy+14,rad=3,base=STEEL)
        scr(r,[px+3,cy-10,px+25,cy+1],col=ac,grid=False)
    # side pump stacks
    for px in (18, W-36):
        coilbank(r,px,cy-22,px+0.001 if False else px, cy) if False else None
    save(r,name)

def battery():
    r=R(1,1); W=r.WU_w
    d=r.d
    d.rounded_rectangle([U(r,14),U(r,14),U(r,W-14),U(r,19)],radius=U(r,1),fill=AMBER+(255,),outline=EDGE+(255,),width=SS)  # bus
    d.rounded_rectangle([U(r,14),U(r,W-19),U(r,W-14),U(r,W-14)],radius=U(r,1),fill=AMBER+(255,),outline=EDGE+(255,),width=SS)
    for row in range(2):
        for col in range(3):
            bx=18+col*30; by=24+row*36
            plate(r,bx,by,bx+22,by+28,rad=3,base=STEEL,rivets=False)
            d.rounded_rectangle([U(r,bx+2),U(r,by+2),U(r,bx+20),U(r,by+7)],radius=U(r,1),fill=mix(STEEL_D,EDGE,0.4)+(255,))
            d.rectangle([U(r,bx+5),U(r,by),U(r,bx+9),U(r,by+3)],fill=(232,96,74,255))
            lv=[0.9,0.65,1.0,0.55,0.8,0.72][row*3+col]
            fh=22*lv
            col2=GREEN if lv>0.6 else AMBER
            d.rounded_rectangle([U(r,bx+3),U(r,by+26-fh),U(r,bx+19),U(r,by+26)],radius=U(r,1),fill=A(col2,220))
            cable(r,[(bx+11,by+28),(bx+11,W-19)])
    save(r,"battery.png")

# ==================================================================== PROPULSION
def _engine_unit(r, cx, topw=26):
    # Clean top-down thruster: solid housing, central turbine, exhaust bell,
    # tight glow. No protruding side pumps, no cartoon flame.
    d=r.d
    d.rounded_rectangle([U(r,cx-topw/2),U(r,10),U(r,cx+topw/2),U(r,56)],radius=U(r,6),fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(U(r,cx-topw/2+3),U(r,12)),(U(r,cx+topw/2-3),U(r,12))],fill=A(STEEL_L,175),width=SS)
    d.line([(U(r,cx-topw/2+3),U(r,54)),(U(r,cx+topw/2-3),U(r,54))],fill=A(EDGE,150),width=SS)
    # flush intake fins on the shoulders
    for side in (-1,1):
        ex=cx+side*topw/2
        for gy in (17,23,29):
            d.line([(U(r,ex-side*2.5),U(r,gy)),(U(r,ex-side*7),U(r,gy))],fill=A(EDGE,150),width=SS)
    # central turbine
    d.ellipse([U(r,cx-10),U(r,20),U(r,cx+10),U(r,40)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS*2)
    for a in range(0,360,45):
        d.line([(U(r,cx),U(r,30)),(U(r,cx+math.cos(math.radians(a))*8),U(r,30+math.sin(math.radians(a))*8))],fill=A(STEEL,190),width=SS)
    d.ellipse([U(r,cx-3),U(r,27),U(r,cx+3),U(r,33)],fill=STEEL_L+(255,),outline=EDGE+(200,),width=SS)
    # combustion band
    d.rounded_rectangle([U(r,cx-topw/2+2),U(r,44),U(r,cx+topw/2-2),U(r,56)],radius=U(r,1),fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,150),width=SS)
    # exhaust bell (clean trapezoid)
    d.polygon([(U(r,cx-topw*0.46),U(r,56)),(U(r,cx+topw*0.46),U(r,56)),(U(r,cx+topw*0.30),U(r,80)),(U(r,cx-topw*0.30),U(r,80))],fill=STEEL_D+(255,),outline=EDGE+(255,))
    d.line([(U(r,cx-topw*0.40),U(r,60)),(U(r,cx+topw*0.40),U(r,60))],fill=A(STEEL,150),width=SS)
    # tight exhaust: glow ring at the mouth + short flame
    g,gd=layer(r)
    gd.ellipse([U(r,cx-topw*0.30),U(r,75),U(r,cx+topw*0.30),U(r,84)],fill=A(ORGL,190))
    gd.polygon([(U(r,cx-topw*0.22),U(r,80)),(U(r,cx+topw*0.22),U(r,80)),(U(r,cx+topw*0.09),U(r,94)),(U(r,cx-topw*0.09),U(r,94))],fill=A(ORG,200))
    gd.polygon([(U(r,cx-topw*0.11),U(r,80)),(U(r,cx+topw*0.11),U(r,80)),(U(r,cx),U(r,92))],fill=A(ORGW,240))
    compose(r,blur(g,1.0))

def standard_engine(cw,name):
    r=R(cw,1); W=r.WU_w
    if cw>=2:
        _engine_unit(r,W*0.30,topw=20); _engine_unit(r,W*0.70,topw=20)
    else:
        _engine_unit(r,W/2,topw=26)
    save(r,name)

def silent_drive():
    r=R(1,1); W=r.WU_w; cx=W/2; d=r.d
    d.rounded_rectangle([U(r,cx-26),U(r,12),U(r,cx+26),U(r,58)],radius=U(r,8),fill=mix(STEEL_D,(70,90,120),0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    for gy in range(20,56,5):
        d.line([(U(r,cx-22),U(r,gy)),(U(r,cx+22),U(r,gy))],fill=A((96,116,150),160),width=SS*2)
    d.polygon([(U(r,cx-18),U(r,58)),(U(r,cx+18),U(r,58)),(U(r,cx+11),U(r,78)),(U(r,cx-11),U(r,78))],fill=mix(STEEL_D,EDGE,0.3)+(255,),outline=EDGE+(255,))
    g,gd=layer(r)
    gd.polygon([(U(r,cx-10),U(r,79)),(U(r,cx+10),U(r,79)),(U(r,cx+5),U(r,92)),(U(r,cx-5),U(r,92))],fill=(90,140,190,150))
    gd.polygon([(U(r,cx-5),U(r,80)),(U(r,cx+5),U(r,80)),(U(r,cx),U(r,90))],fill=(160,200,240,210))
    compose(r,blur(g,2.2))
    save(r,"silent_drive.png")

# ==================================================================== LIFE SUPPORT
def oxygen(cw,name):
    r=R(cw,1); W,Hh=r.WU_w,r.WU_h; d=r.d
    n=2 if cw<2 else 4
    for i in range(n):
        cx=W*(i+0.5)/n
        cylinder(r,cx,18,Hh-24,18,STEEL,fluid=GREEN)
        d.rectangle([U(r,cx-6),U(r,Hh*0.42),U(r,cx+6),U(r,Hh*0.42+5)],fill=A(GREEN,180))
        d.ellipse([U(r,cx-4),U(r,14),U(r,cx+4),U(r,22)],fill=(10,24,28,255),outline=EDGE+(200,),width=SS)
        d.line([(U(r,cx),U(r,18)),(U(r,cx+2),U(r,15))],fill=GREEN_HI+(255,),width=SS)
    plate(r,W/2-16,Hh-22,W/2+16,Hh-8,rad=3,base=STEEL,rivets=False)
    d.ellipse([U(r,W/2-8),U(r,Hh-20),U(r,W/2+4),U(r,Hh-10)],fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(200,),width=SS)
    save(r,name)

def life_support():
    r=R(1,1); W=r.WU_w; d=r.d
    for i in range(3):
        cx=W*(0.24+i*0.26)
        plate(r,cx-10,18,cx+10,52,rad=4,base=STEEL,rivets=False)
        for gy in (26,32,38,44): d.line([(U(r,cx-8),U(r,gy)),(U(r,cx+8),U(r,gy))],fill=A(GREEN_D,160),width=SS)
        d.rectangle([U(r,cx-6),U(r,20),U(r,cx+6),U(r,24)],fill=A(GREEN_D,200))
    r.pipe(U(r,16),U(r,62),U(r,W-16),U(r,62),w=5,col=STEEL_L)
    r.pipe(U(r,20),U(r,62),U(r,20),U(r,78),w=5,col=STEEL_L)
    scr(r,[W*0.42,70,W*0.74,86],col=GREEN)
    save(r,"life_support.png")

# ==================================================================== WEAPONS (barrel UP)
def point_defense():
    # Clean compact CIWS turret — deliberately simpler than the railgun.
    r=R(1,1); W=r.WU_w; cx=W/2; cy=W*0.55; d=r.d
    # circular turret base (two clean rings, no gear teeth / clutter)
    d.ellipse([U(r,cx-23),U(r,cy-23),U(r,cx+23),U(r,cy+23)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([U(r,cx-23),U(r,cy-23),U(r,cx+23),U(r,cy+23)],20,160,fill=A(STEEL_L,160),width=SS)
    d.ellipse([U(r,cx-15),U(r,cy-15),U(r,cx+15),U(r,cy+15)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS*2)
    # compact gun housing on the ring
    d.rounded_rectangle([U(r,cx-11),U(r,cy-7),U(r,cx+11),U(r,cy+13)],radius=U(r,3),fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(U(r,cx-9),U(r,cy-5)),(U(r,cx+9),U(r,cy-5))],fill=A(STEEL_L,160),width=SS)
    # ammo drum behind the housing
    d.ellipse([U(r,cx-9),U(r,cy+7),U(r,cx+9),U(r,cy+22)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS)
    d.arc([U(r,cx-9),U(r,cy+7),U(r,cx+9),U(r,cy+22)],200,340,fill=A(STEEL_L,120),width=SS)
    # twin clean barrels pointing up
    for ox in (-5,5):
        barrel(r,cx+ox,5,cy-5,4)
    # two small status lamps
    d.ellipse([U(r,cx-20),U(r,cy-1),U(r,cx-17),U(r,cy+2)],fill=(120,225,150,255))
    d.ellipse([U(r,cx+17),U(r,cy-1),U(r,cx+20),U(r,cy+2)],fill=(232,96,74,255))
    save(r,"point_defense.png")

def railgun(cw,name):
    r=R(cw,1); W,Hh=r.WU_w,r.WU_h; cx=W/2; cy=Hh*0.58
    plate(r,14 if cw>=2 else 26, 30, W-(14 if cw>=2 else 26), Hh-8, rad=8, hazard=True)
    # recessed well
    r.d.rounded_rectangle([U(r,cx-34),U(r,34),U(r,cx+34),U(r,Hh-20)],radius=U(r,6),fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    r.d.arc([U(r,cx-34),U(r,34),U(r,cx+34),U(r,Hh-20)],190,350,fill=EDGE+(255,),width=SS*2)
    # coil banks
    for wx in (cx-58 if cw>=2 else cx-22, cx+42 if cw>=2 else cx+12):
        coilbank(r,wx,42,Hh-24,16 if cw>=2 else 10)
    # ammo drums for 2x1
    if cw>=2:
        for ax in (20,W-46):
            plate(r,ax,Hh-32,ax+26,Hh-14,rad=3,base=mix(STEEL_D,STEEL,0.5),rivets=False)
            for i in range(3):
                r.d.rounded_rectangle([U(r,ax+3+i*7),U(r,Hh-29),U(r,ax+7+i*7),U(r,Hh-17)],radius=U(r,1),fill=BRASS+(255,),outline=EDGE+(255,),width=SS)
    # twin rails + energized rungs to top
    for rx in (cx-8,cx+4):
        r.d.rounded_rectangle([U(r,rx),U(r,0),U(r,rx+4),U(r,cy)],radius=U(r,1),fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
    barrel(r,cx,0,cy,10,rail=True)
    # breech + capacitor
    r.d.polygon([(U(r,cx-11),U(r,cy-4)),(U(r,cx+11),U(r,cy-4)),(U(r,cx+15),U(r,cy+22)),(U(r,cx-15),U(r,cy+22))],fill=STEEL+(255,),outline=EDGE+(255,))
    softglow(r,cx,cy+10,8,ENERGY,ENERGY_HI)
    save(r,name)

def torpedo_tube():
    r=R(1,1); W,Hh=r.WU_w,r.WU_h; d=r.d
    # rear frame
    d.rounded_rectangle([U(r,24),U(r,14),U(r,W-24),U(r,22)],radius=U(r,1),fill=mix(STEEL_D,STEEL,0.5)+(255,),outline=EDGE+(255,),width=SS)
    # closed tube
    plate(r,28,10,58,Hh-16,rad=6,base=STEEL,rivets=False)
    d.line([(U(r,43),U(r,14)),(U(r,43),U(r,Hh-20))],fill=A(EDGE,200),width=SS)
    d.rectangle([U(r,38),U(r,Hh-34),U(r,48),U(r,Hh-28)],fill=(210,180,70,255))
    # open tube w/ missile
    d.rounded_rectangle([U(r,68),U(r,10),U(r,98),U(r,Hh-16)],radius=U(r,6),fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([U(r,68),U(r,10),U(r,98),U(r,Hh-16)],190,350,fill=EDGE+(255,),width=SS*2)
    mx=83
    d.rounded_rectangle([U(r,mx-6),U(r,26),U(r,mx+6),U(r,Hh-36)],radius=U(r,3),fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
    d.polygon([(U(r,mx-6),U(r,26)),(U(r,mx+6),U(r,26)),(U(r,mx),U(r,10))],fill=RED+(255,))
    d.rectangle([U(r,mx-6),U(r,40),U(r,mx+6),U(r,44)],fill=RED+(255,))
    d.polygon([(U(r,mx-6),U(r,Hh-40)),(U(r,mx-11),U(r,Hh-30)),(U(r,mx-6),U(r,Hh-30))],fill=STEEL+(255,),outline=EDGE+(200,))
    d.polygon([(U(r,mx+6),U(r,Hh-40)),(U(r,mx+11),U(r,Hh-30)),(U(r,mx+6),U(r,Hh-30))],fill=STEEL+(255,),outline=EDGE+(200,))
    plate(r,44,Hh-34,66,Hh-22,rad=2,base=mix(STEEL_D,STEEL,0.5),rivets=False)
    save(r,"torpedo_tube.png")

def mine_layer():
    r=R(1,1); W,Hh=r.WU_w,r.WU_h; d=r.d
    for i in range(3):
        cx=W*(0.26+i*0.24)
        plate(r,cx-8,20,cx+8,Hh-18,rad=3,base=mix(STEEL_D,STEEL,0.5),rivets=False)
        d.polygon([(U(r,cx-6),U(r,26)),(U(r,cx+6),U(r,26)),(U(r,cx),U(r,16))],fill=AMBER+(255,))
        d.rectangle([U(r,cx-6),U(r,30),U(r,cx+6),U(r,34)],fill=RED+(255,))
        d.ellipse([U(r,cx-4),U(r,Hh-34),U(r,cx+4),U(r,Hh-26)],fill=(232,96,74,255),outline=EDGE+(200,),width=SS)
        for a in range(0,360,90):
            d.line([(U(r,cx),U(r,Hh-30)),(U(r,cx+math.cos(math.radians(a))*6),U(r,Hh-30+math.sin(math.radians(a))*6))],fill=STEEL_L+(255,),width=SS)
    save(r,"mine_layer.png")

def salvage_arm():
    r=R(1,1); W,Hh=r.WU_w,r.WU_h; bx,by=W/2,84; d=r.d
    plate(r,bx-20,80,bx+20,Hh-12,rad=4,base=STEEL,rivets=False)
    d.ellipse([U(r,bx-12),U(r,72),U(r,bx+12),U(r,88)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    j1=(bx,78); j2=(W*0.36,48); j3=(W*0.60,22)
    for a,b in [(j1,j2),(j2,j3)]:
        d.line([(U(r,a[0]),U(r,a[1])),(U(r,b[0]),U(r,b[1]))],fill=EDGE+(255,),width=SS*5)
        d.line([(U(r,a[0]),U(r,a[1])),(U(r,b[0]),U(r,b[1]))],fill=STEEL_L+(255,),width=SS*2)
    for j in (j1,j2,j3):
        d.ellipse([U(r,j[0]-4),U(r,j[1]-4),U(r,j[0]+4),U(r,j[1]+4)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    cxx,cyy=j3
    for s in (-1,1):
        d.line([(U(r,cxx),U(r,cyy)),(U(r,cxx+s*8),U(r,cyy-10))],fill=AMBER+(255,),width=SS*2)
        d.line([(U(r,cxx+s*8),U(r,cyy-10)),(U(r,cxx+s*11),U(r,cyy-6))],fill=AMBER+(255,),width=SS*2)
    save(r,"salvage_arm.png")

# ==================================================================== SENSORS
def sonar_array():
    r=R(1,1); W=r.WU_w; cx=W/2; cy=44; d=r.d
    plate(r,44,86,W-44,W-16,rad=4,base=STEEL,rivets=False)
    d.rounded_rectangle([U(r,cx-5),U(r,70),U(r,cx+5),U(r,88)],radius=U(r,1.5),fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    for rr in (36,29,20,10):
        d.ellipse([U(r,cx-rr),U(r,cy-rr*0.75),U(r,cx+rr),U(r,cy+rr*0.75)],
                  fill=mix(STEEL,STEEL_D,(36-rr)/36)+(255,) if rr>10 else (10,26,30,255),outline=EDGE+(255,),width=SS)
    d.arc([U(r,cx-29),U(r,cy-22),U(r,cx+29),U(r,cy+22)],20,160,fill=A(STEEL_L,180),width=SS)
    d.line([(U(r,cx),U(r,cy)),(U(r,cx),U(r,cy-34))],fill=STEEL_L+(255,),width=SS*2)
    softglow(r,cx,cy-36,5,CYAN,(210,245,255))
    save(r,"sonar_array.png")

def passive_sonar(cw,name):
    r=R(cw,1); W,Hh=r.WU_w,r.WU_h; d=r.d
    n=1 if cw<2 else 2
    for k in range(n):
        base=W*(k+0.5)/n
        for i in range(4):
            yy=20+i*13
            d.rounded_rectangle([U(r,base-20),U(r,yy),U(r,base+20),U(r,yy+7)],radius=U(r,2),fill=mix(STEEL_D,STEEL,0.5)+(255,),outline=EDGE+(255,),width=SS)
            for j in range(5):
                dx=base-16+j*8
                d.ellipse([U(r,dx-1.5),U(r,yy+1.5),U(r,dx+1.5),U(r,yy+5)],fill=A(CYAN,200))
    scr(r,[W*0.32,Hh-24,W*0.68,Hh-10],col=CYAN)
    save(r,name)

def depth_sensor():
    r=R(1,1); W=r.WU_w; cx=W/2; cy=54; d=r.d
    d.rounded_rectangle([U(r,cx-4),U(r,84),U(r,cx+4),U(r,W-10)],radius=U(r,1),fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    plate(r,26,22,W-26,84,rad=6,base=STEEL)
    d.ellipse([U(r,cx-22),U(r,cy-22),U(r,cx+22),U(r,cy+22)],fill=(8,24,28,255),outline=EDGE+(255,),width=SS*2)
    for rr in (16,10,5): d.ellipse([U(r,cx-rr),U(r,cy-rr),U(r,cx+rr),U(r,cy+rr)],outline=A(CYAN,110),width=SS)
    d.line([(U(r,cx),U(r,cy)),(U(r,cx+15),U(r,cy-11))],fill=CYAN+(255,),width=SS*2)
    g,gd=layer(r); gd.pieslice([U(r,cx-16),U(r,cy-16),U(r,cx+16),U(r,cy+16)],-40,0,fill=A(CYAN,60)); compose(r,blur(g,0.6))
    d=r.d
    d.ellipse([U(r,cx+7),U(r,cy-13),U(r,cx+9),U(r,cy-11)],fill=(120,225,150,255))
    for px in (18,W-30):
        plate(r,px,40,px+12,68,rad=2,base=mix(STEEL_D,STEEL,0.5),rivets=False)
        d.ellipse([U(r,px+3),U(r,44),U(r,px+9),U(r,50)],fill=(8,24,28,255)); d.point((U(r,px+5),U(r,46)),fill=(210,245,255,255))
    save(r,"depth_sensor.png")

# ==================================================================== COMMAND (console cluster on a partial dais)
def navigation(cw,ch,name):
    r=R(cw,ch); W,Hh=r.WU_w,r.WU_h; d=r.d
    d.rounded_rectangle([U(r,14),U(r,Hh*0.20),U(r,W-14),U(r,Hh-12)],radius=U(r,10),fill=mix(STEEL_D,EDGE,0.35)+(255,),outline=EDGE+(255,),width=SS*2)
    d.rounded_rectangle([U(r,18),U(r,Hh*0.20+4),U(r,W-18),U(r,Hh-16)],radius=U(r,8),outline=A(STEEL,120),width=SS)
    if cw>=3:
        plate(r,W*0.14,Hh*0.10,W*0.86,Hh*0.14+18,rad=4,base=STEEL,rivets=False)
        scr(r,[W*0.16,Hh*0.12,W*0.84,Hh*0.28],col=ENERGY)
        for fx in (0.20,0.80):
            plate(r,W*fx-14,Hh*0.50,W*fx+14,Hh*0.66,rad=3,base=STEEL,rivets=False)
            scr(r,[W*fx-11,Hh*0.52,W*fx+11,Hh*0.62],col=CYAN,grid=False)
            chair(r,W*fx,Hh*0.80)
        chair(r,W/2,Hh*0.62,big=True)
    elif cw>=2:
        for fx in (0.16,0.30):
            plate(r,W*fx-8,Hh*0.20,W*fx+8,Hh*0.74,rad=2,base=mix(STEEL_D,STEEL,0.4),rivets=False)
            for gy in range(6):
                d.rounded_rectangle([U(r,W*fx-5),U(r,Hh*0.26+gy*8),U(r,W*fx-2),U(r,Hh*0.26+gy*8+3)],radius=U(r,0.5),fill=(120,225,150,255))
        scr(r,[W*0.46,Hh*0.22,W*0.90,Hh*0.66],col=ENERGY)
        chair(r,W*0.70,Hh*0.82)
    else:
        plate(r,20,Hh*0.14,W-20,Hh*0.42,rad=6,base=STEEL,rivets=False)
        scr(r,[26,Hh*0.18,W-26,Hh*0.36],col=CYAN)
        for i,c in enumerate([AMBER,GREEN,CYAN]):
            d.rounded_rectangle([U(r,34+i*10),U(r,Hh*0.44),U(r,39+i*10),U(r,Hh*0.47)],radius=U(r,0.5),fill=c+(255,))
        for px in (20,W-32):
            plate(r,px,Hh*0.50,px+12,Hh*0.72,rad=2,base=STEEL,rivets=False)
            scr(r,[px+2,Hh*0.53,px+10,Hh*0.63],col=CYAN,grid=False)
        chair(r,W/2,Hh*0.74)
    save(r,name)

# ==================================================================== STORAGE-machinery
def ballast_tank():
    # Industrial fuel drum — solid metal, hazard band, flammable mark, gauge,
    # valves. NOT a translucent fluid window ("glass of water").
    r=R(1,1); W,Hh=r.WU_w,r.WU_h; cx,cy=W/2,Hh/2; d=r.d
    x0,x1,y0,y1=cx-19,cx+19,20,Hh-18
    d.rounded_rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],radius=U(r,9),fill=mix(STEEL,(122,108,84),0.30)+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(U(r,x0+2.5),U(r,y0+5)),(U(r,x0+2.5),U(r,y1-5))],fill=A(STEEL_L,175),width=SS)
    d.line([(U(r,x1-2.5),U(r,y0+5)),(U(r,x1-2.5),U(r,y1-5))],fill=A(EDGE,150),width=SS)
    # reinforcing bands
    for t in (0.28,0.72):
        by=y0+(y1-y0)*t
        d.rectangle([U(r,x0),U(r,by-2),U(r,x1),U(r,by+2)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,150),width=SS)
    # hazard / flammable chevron band around the middle
    hy0,hy1=cy-4,cy+5
    d.rectangle([U(r,x0+1),U(r,hy0),U(r,x1-1),U(r,hy1)],fill=(214,150,54,255))
    step=U(r,5); sx=U(r,x0)-step; band=U(r,hy1-hy0)
    while sx<U(r,x1):
        d.polygon([(sx,U(r,hy1)),(sx+step*0.5,U(r,hy1)),(sx+step*0.5+band,U(r,hy0)),(sx+band,U(r,hy0))],fill=(36,30,24,255)); sx+=step
    d.rectangle([U(r,x0+1),U(r,hy0),U(r,x1-1),U(r,hy1)],outline=EDGE+(255,),width=SS)
    # flammable triangle warning near the top
    tcx,tcy=cx,y0+(y1-y0)*0.15
    d.polygon([(U(r,tcx),U(r,tcy-5)),(U(r,tcx-5),U(r,tcy+4)),(U(r,tcx+5),U(r,tcy+4))],fill=(234,192,72,255),outline=EDGE+(255,))
    d.line([(U(r,tcx),U(r,tcy-2)),(U(r,tcx),U(r,tcy+1))],fill=EDGE+(255,),width=SS)
    d.ellipse([U(r,tcx-0.5),U(r,tcy+2.5),U(r,tcx+0.5),U(r,tcy+3.5)],fill=EDGE+(255,))
    # fuel gauge dial (lower)
    gy=y0+(y1-y0)*0.84
    d.ellipse([U(r,cx-6),U(r,gy-6),U(r,cx+6),U(r,gy+6)],fill=(20,26,30,255),outline=STEEL_D+(255,),width=SS)
    d.line([(U(r,cx),U(r,gy)),(U(r,cx+4),U(r,gy-3))],fill=(234,152,60,255),width=SS)
    d.ellipse([U(r,cx-1),U(r,gy-1),U(r,cx+1),U(r,gy+1)],fill=STEEL_L+(255,))
    # top fill cap + side valve fittings
    d.rounded_rectangle([U(r,cx-6),U(r,y0-4),U(r,cx+6),U(r,y0+3)],radius=U(r,1.5),fill=mix(STEEL_D,STEEL,0.5)+(255,),outline=EDGE+(255,),width=SS)
    for side in (-1,1):
        vx=cx+side*19
        r.pipe(U(r,vx),U(r,y1-10),U(r,vx+side*5),U(r,y1-10),w=3,col=STEEL_D)
        d.ellipse([U(r,vx+side*5-2.5),U(r,y1-12.5),U(r,vx+side*5+2.5),U(r,y1-7.5)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS)
    save(r,"ballast_tank.png")

def research_lab(cw,name):
    r=R(cw,1); W,Hh=r.WU_w,r.WU_h; big=cw>=2; d=r.d
    tcx=W*(0.26 if big else 0.33); tw=11 if big else 9
    cylinder(r,tcx,18,Hh-18,tw*2,STEEL,fluid=(70,180,170))
    d.ellipse([U(r,tcx-6),U(r,Hh/2-6),U(r,tcx+6),U(r,Hh/2+6)],fill=(24,48,54,220))
    for a in range(5):
        ang=math.radians(20+a*32)
        d.line([(U(r,tcx),U(r,Hh/2)),(U(r,tcx+math.cos(ang)*9),U(r,Hh/2+math.sin(ang)*9))],fill=(24,48,54,200),width=SS*2)
    bx0=W*(0.50 if big else 0.52)
    plate(r,bx0,Hh-30,W-16,Hh-16,rad=2,base=STEEL,rivets=False)
    for i,cc in enumerate([(120,220,150),(230,180,90),(160,120,220)]):
        d.rounded_rectangle([U(r,bx0+4+i*8),U(r,Hh-38),U(r,bx0+8+i*8),U(r,Hh-30)],radius=U(r,0.6),fill=A(cc,220),outline=EDGE+(200,),width=SS)
    scr(r,[bx0,22,W-16,Hh-40],col=(120,200,190))
    save(r,name)

# ==================================================================== UTILITY
def repair(cw,name):
    r=R(cw,1); W,Hh=r.WU_w,r.WU_h; big=cw>=2; d=r.d
    for i in range(5):
        tx=W*(0.14+i*0.11)
        d.line([(U(r,tx),U(r,10)),(U(r,tx),U(r,15))],fill=STEEL_D+(255,),width=SS)
        d.rounded_rectangle([U(r,tx-2),U(r,10),U(r,tx+2),U(r,14)],radius=U(r,0.5),fill=([AMBER,STEEL_L,AMBER,STEEL_L,AMBER][i])+(255,),outline=EDGE+(200,),width=SS)
    plate(r,W*0.10,18,W*(0.60 if big else 0.90),30,rad=2,base=STEEL,rivets=False)
    vent(r,W*0.14,22,W*0.30,27)
    plate(r,W*0.12,Hh-30,W*0.36,Hh-14,rad=2,base=mix(STEEL_D,STEEL,0.5),rivets=False)
    if big:
        px,py=W*0.76,Hh/2+4
        d.ellipse([U(r,px-20),U(r,py-14),U(r,px+20),U(r,py+14)],outline=A(ENERGY,120),width=SS)
        plate(r,px-10,py-7,px+10,py+7,rad=3,base=mix(STEEL_D,STEEL,0.4),rivets=False)
        for (ox,oy) in [(-13,-10),(13,-10),(-13,10),(13,10)]:
            d.ellipse([U(r,px+ox-4),U(r,py+oy-4),U(r,px+ox+4),U(r,py+oy+4)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
        softglow(r,px,py,5,CYAN,(210,245,255))
    else:
        bx,by=W*0.70,Hh-18; j2=(W*0.62,Hh/2)
        d.line([(U(r,bx),U(r,by)),(U(r,j2[0]),U(r,j2[1]))],fill=EDGE+(255,),width=SS*5)
        d.line([(U(r,bx),U(r,by)),(U(r,j2[0]),U(r,j2[1]))],fill=STEEL_L+(255,),width=SS*2)
        d.line([(U(r,j2[0]),U(r,j2[1])),(U(r,W*0.80),U(r,Hh*0.34))],fill=EDGE+(255,),width=SS*4)
        d.line([(U(r,j2[0]),U(r,j2[1])),(U(r,W*0.80),U(r,Hh*0.34))],fill=STEEL_L+(255,),width=SS)
        d.ellipse([U(r,bx-8),U(r,by-5),U(r,bx+8),U(r,by+6)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    save(r,name)

def docking(cw,name):
    r=R(cw,cw); W,Hh=r.WU_w,r.WU_h; cx,cy=W/2,Hh/2; d=r.d
    rad=min(W,Hh)/2-14
    d.ellipse([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],20,160,fill=A(STEEL_L,150),width=SS)
    d.ellipse([U(r,cx-rad+5),U(r,cy-rad+5),U(r,cx+rad-5),U(r,cy+rad-5)],fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    for a in range(0,360,45):
        ex=cx+math.cos(math.radians(a))*rad*0.92; ey=cy+math.sin(math.radians(a))*rad*0.92
        ix=cx+math.cos(math.radians(a))*rad*0.62; iy=cy+math.sin(math.radians(a))*rad*0.62
        d.line([(U(r,ix),U(r,iy)),(U(r,ex),U(r,ey))],fill=STEEL_L+(255,),width=SS*3)
    d.ellipse([U(r,cx-rad*0.5),U(r,cy-rad*0.5),U(r,cx+rad*0.5),U(r,cy+rad*0.5)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    for a in range(0,360,60):
        d.line([(U(r,cx),U(r,cy)),(U(r,cx+math.cos(math.radians(a))*rad*0.5),U(r,cy+math.sin(math.radians(a))*rad*0.5))],fill=A(EDGE,180),width=SS)
    d.ellipse([U(r,cx-4),U(r,cy-4),U(r,cx+4),U(r,cy+4)],fill=CAUTION+(255,),outline=EDGE+(255,),width=SS)
    save(r,name)

def floodlight():
    r=R(1,1); W=r.WU_w; cx=W/2; d=r.d
    g,gd=layer(r)
    gd.polygon([(U(r,cx-12),U(r,34)),(U(r,cx+12),U(r,34)),(U(r,cx+20),U(r,4)),(U(r,cx-20),U(r,4))],fill=(255,242,196,70))
    compose(r,blur(g,2.4)); d=r.d
    d.rounded_rectangle([U(r,cx-12),U(r,80),U(r,cx+12),U(r,94)],radius=U(r,2),fill=STEEL+(255,),outline=EDGE+(255,),width=SS)  # mount tab
    d.rounded_rectangle([U(r,cx-3),U(r,58),U(r,cx+3),U(r,80)],radius=U(r,1),fill=STEEL+(255,),outline=EDGE+(255,),width=SS)   # arm
    d.ellipse([U(r,cx-4),U(r,54),U(r,cx+4),U(r,62)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)  # joint
    d.rounded_rectangle([U(r,cx-15),U(r,36),U(r,cx+15),U(r,58)],radius=U(r,3),fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)  # head
    for gy in (40,44,48,52): d.line([(U(r,cx+8),U(r,gy)),(U(r,cx+13),U(r,gy))],fill=A(EDGE,150),width=SS)
    d.ellipse([U(r,cx-11),U(r,40),U(r,cx+1),U(r,52)],fill=(255,246,210,255),outline=EDGE+(200,),width=SS)
    d.ellipse([U(r,cx-8),U(r,43),U(r,cx-2,),U(r,49)],fill=(255,253,240,255))
    save(r,"floodlight.png")

# ==================================================================== run
if __name__=="__main__":
    small_reactor(); large_reactor(2,1,"large_reactor_2x1.png"); large_reactor(3,3,"large_reactor_3x3.png"); battery()
    standard_engine(1,"standard_engine.png"); standard_engine(2,"standard_engine_2x1.png"); silent_drive()
    oxygen(1,"oxygen_scrubber.png"); oxygen(2,"oxygen_scrubber_2x1.png"); life_support()
    point_defense(); railgun(1,"railgun.png"); railgun(2,"railgun_2x1.png"); torpedo_tube(); mine_layer(); salvage_arm()
    sonar_array(); passive_sonar(1,"passive_sonar.png"); passive_sonar(2,"passive_sonar_2x1.png"); depth_sensor()
    navigation(1,1,"navigation.png"); navigation(2,1,"navigation_2x1.png"); navigation(3,2,"navigation_3x2.png")
    ballast_tank(); research_lab(1,"research_lab.png"); research_lab(2,"research_lab_2x1.png")
    repair(1,"repair_station.png"); repair(2,"repair_station_2x1.png")
    docking(1,"docking_port.png"); docking(3,"docking_port_3x3.png"); floodlight()
    print("smooth machinery set done")
