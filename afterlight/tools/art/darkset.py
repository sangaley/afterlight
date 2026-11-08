#!/usr/bin/env python3
"""Full module set in the approved DARK + block-filling + Cosmoteer-detail style.
Each module is its own composition (no copy-paste), drawn at its true footprint
(multi-cell = new art, never a stretch), directional 1x1 parts (engines/guns)
protrude past the block. Then everything is pixelated. Reuses cosmo's dark
palette + detail helpers."""
from cosmo import *          # dark palette + bolt/sphere_shade/bevel_rect/core/grime/soot/
                             # hazard_band/block_backing/metal_noise/rim3d + R/U/save/layer/compose
from cosmo import reactor as _small_reactor, thruster_overhang as _thruster
from rooms_gen import fname, JOBS
import math, random, os, shutil
from PIL import Image, ImageDraw

# muted accents (used as contained glows / small lights, not big flat fills)
GRN=(96,200,128); CY=(108,198,224); AMB=(212,168,84); RD=(206,74,60); TEALC=(78,166,162); VIO=(150,122,212)
WOOD=(96,74,52); WOODL=(122,96,66); LINEN=(150,158,170); BLANK=(52,76,120); SKIN=(150,120,96)

def P(r,x,y): return (U(r,x),U(r,y))
def rr(r,box,**k): r.d.rounded_rectangle([U(r,box[0]),U(r,box[1]),U(r,box[2]),U(r,box[3])],**k)

def fill_block(r,x0,y0,x1,y1,seed,base=None,haz_bottom=False):
    if base is None: base=mix(STEEL,STEEL_D,0.5)
    bevel_rect(r,x0,y0,x1,y1,base,rad=8)
    metal_noise(r,(x0+x1)/2,(y0+y1)/2,(x1-x0)/2,seed)
    grime(r,x0+2,y0+2,x1-2,y1-2,seed+"g")
    for (bx,by) in [(x0+6,y0+6),(x1-6,y0+6),(x0+6,y1-6),(x1-6,y1-6)]: bolt(r,bx,by,1.7)
    if haz_bottom: hazard_band(r,x0+4,y1-8,x1-4,y1-3)

def led(r,x,y,c,rad=1.6):
    r.d.ellipse([U(r,x-rad),U(r,y-rad),U(r,x+rad),U(r,y+rad)],fill=c+(255,))
    r.d.ellipse([U(r,x-rad*0.4),U(r,y-rad*0.6),U(r,x+rad*0.2),U(r,y)],fill=A(mix(c,(255,255,255),0.6),230))

def dscreen(r,box,col=CY,wave=True):
    b=[U(r,box[0]),U(r,box[1]),U(r,box[2]),U(r,box[3])]
    r.d.rounded_rectangle(b,radius=U(r,1.5),fill=(6,14,18,255),outline=EDGE+(255,),width=SS)
    x0,y0,x1,y1=b; yb=(y0+y1)/2; sp=x1-x0
    gy=y0
    while gy<y1: r.d.line([(x0,gy),(x1,gy)],fill=A(col,26),width=SS); gy+=U(r,2.6)
    if wave:
        r.d.line([(x0+U(r,1),yb),(x0+sp*0.3,yb-sp*0.09),(x0+sp*0.5,yb+sp*0.09),(x0+sp*0.7,yb-sp*0.03),(x1-U(r,1),yb)],fill=col+(255,),width=SS)
    led(r,box[0]+2,box[3]-2,GRN,1.1)

def small_glow(r,cx,cy,rad,col,hi):
    g,gd=layer(r); N=16
    for i in range(N,0,-1):
        t=i/N; rrad=rad*t
        gd.ellipse([U(r,cx-rrad),U(r,cy-rrad),U(r,cx+rrad),U(r,cy+rrad)],fill=mix(hi,mix(col,DARK,0.4),t)+(int(30+120*(1-t)),))
    gd.ellipse([U(r,cx-rad*0.25),U(r,cy-rad*0.25),U(r,cx+rad*0.25),U(r,cy+rad*0.25)],fill=hi+(255,))
    compose(r,blur(g,1.2))

def coilbank(r,x,y0,y1,w):
    rr(r,[x,y0,x+w,y1],radius=U(r,1.5),fill=mix(STEEL_D,DARK,0.3)+(255,),outline=EDGE+(255,),width=SS)
    yy=y0+1.5;k=0
    while yy<y1-1.5:
        rr(r,[x+1,yy,x+w-1,yy+2],radius=U(r,0.5),fill=(COPL if k%2==0 else COP)+(255,))
        yy+=3.2;k+=1

def pipe(r,x0,y0,x1,y1,w=4,col=None):
    if col is None: col=STEEL_D
    r.d.line([P(r,x0,y0),P(r,x1,y1)],fill=EDGE+(255,),width=int(SS*w))
    r.d.line([P(r,x0,y0),P(r,x1,y1)],fill=col+(255,),width=max(SS,int(SS*w*0.5)))

def cyl(r,cx,y0,y1,w,base,accent=None):
    x0,x1=cx-w/2,cx+w/2
    rr(r,[x0,y0,x1,y1],radius=U(r,w/2),fill=base+(255,),outline=EDGE+(255,),width=SS*2)
    r.d.line([P(r,x0+1.2,y0+2),P(r,x0+1.2,y1-2)],fill=A(STEEL_L,150),width=SS)
    r.d.line([P(r,x1-1.2,y0+2),P(r,x1-1.2,y1-2)],fill=A(EDGE,160),width=SS)
    for t in (0.3,0.6):
        yy=y0+(y1-y0)*t; r.d.line([P(r,x0,yy),P(r,x1,yy)],fill=A(EDGE,140),width=SS)
    if accent:
        yy=y0+(y1-y0)*0.5; r.d.rectangle([U(r,x0),U(r,yy-1.5),U(r,x1),U(r,yy+2)],fill=A(accent,150))

# ---- overhang canvas (1x1): extend TOP (weapon) or BOTTOM (engine) ----
def Rext2(ext_disp, side):
    r=Room(1,1); art_ext=ext_disp*(r.WU_w/60.0)
    r.WU_h=r.WU_h+art_ext; r.H=int(r.WU_h*FINAL*SS)
    r.img=Image.new("RGBA",(r.W,r.H),(0,0,0,0)); r.d=ImageDraw.Draw(r.img)
    if side=='bottom': return r, 0.0, 126.0, art_ext      # block [0,126], ext below
    else:              return r, art_ext, art_ext+126.0, art_ext  # block below, ext [0,art_ext]

# =====================================================================
# POWER
def large_reactor(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h; cx,cy=W/2,H/2
    fill_block(r,6,6,W-6,H-6,"lr",base=mix(STEEL,STEEL_D,0.55))
    for fy in (H*0.22,H*0.78): pipe(r,14,fy,W-14,fy,5,STEEL_D)
    rad=min(W,H)/2-24
    sphere_shade(r,cx,cy,rad,mix(STEEL_D,DARK,0.3));
    for a0 in range(0,360,40):
        r.d.arc([U(r,cx-rad+6),U(r,cy-rad+6),U(r,cx+rad-6),U(r,cy+rad-6)],a0+5,a0+33,fill=COP+(255,),width=SS*4)
    r.d.ellipse([U(r,cx-rad+15),U(r,cy-rad+15),U(r,cx+rad-15),U(r,cy+rad-15)],fill=mix(DARK,COLD,0.25)+(255,),outline=EDGE+(255,),width=SS*2)
    core(r,cx,cy,rad-17)
    if cw>=3:
        for (fx,fy) in [(0.22,0.24),(0.78,0.24),(0.22,0.76),(0.78,0.76)]: small_glow(r,W*fx,H*fy,9,COLD,HOT)
    for px,ac in ((16,COREBLUE),(W-16-26,AMB)):
        fill_block(r,px,cy-13,px+26,cy+13,"lrc"+str(int(px)),base=mix(STEEL_D,DARK,0.2)); dscreen(r,[px+3,cy-9,px+23,cy+1],col=ac)
    save(r,fn)

def battery(cw,ch,fn):
    r=R(1,1); W=r.WU_w
    fill_block(r,6,6,W-6,W-6,"bat")
    rr(r,[14,12,W-14,17],radius=U(r,1),fill=AMB+(255,),outline=EDGE+(255,),width=SS)
    rr(r,[14,W-17,W-14,W-12],radius=U(r,1),fill=mix(AMB,DARK,0.3)+(255,),outline=EDGE+(255,),width=SS)
    for row in range(2):
        for col in range(3):
            bx=20+col*30; by=24+row*36
            rr(r,[bx,by,bx+22,by+28],radius=U(r,3),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS*2)
            rr(r,[bx+2,by+2,bx+20,by+7],radius=U(r,1),fill=DARK+(255,))
            led(r,bx+6,by+2,RD,1.3); led(r,bx+16,by+2,mix(STEEL,DARK,0.3),1.3)
            lv=[0.9,0.6,1.0,0.5,0.8,0.7][row*3+col]; fh=22*lv
            rr(r,[bx+3,by+26-fh,bx+19,by+26],radius=U(r,1),fill=A(GRN if lv>0.6 else AMB,220))
            pipe(r,bx+11,by+28,bx+11,W-17,3)
    save(r,fn)

# PROPULSION
def silent_drive(cw,ch,fn):
    r,by0,by1,ext=Rext2(24,'bottom'); W=r.WU_w; cx=W/2; BE=by1
    block_backing(r,BE-4)
    fill_block(r,6,6,W-6,BE-5,"sil",base=mix(STEEL_D,(58,66,86),0.35))
    for gy in range(20,int(BE-20),6):  # sound baffles
        r.d.line([P(r,16,gy),P(r,W-16,gy)],fill=A((70,86,120),150),width=SS*2)
    rr(r,[cx-24,20,cx+24,BE-22],radius=U(r,4),outline=A((90,110,150),90),width=SS)
    # shrouded nozzle protruding, muted blue
    bt,bb=BE-4,BE+30
    r.d.polygon([P(r,cx-26,bt),P(r,cx+26,bt),P(r,cx+15,bb),P(r,cx-15,bb)],fill=mix(STEEL_D,DARK,0.4)+(255,),outline=EDGE+(255,))
    g,gd=layer(r)
    gd.polygon([P(r,cx-11,bb),P(r,cx+11,bb),P(r,cx,bb+18)],fill=(90,150,210,150))
    gd.polygon([P(r,cx-5,bb),P(r,cx+5,bb),P(r,cx,bb+12)],fill=(170,205,240,210))
    compose(r,blur(g,2.4))
    save(r,fn)

def engine_2x1(cw,ch,fn):   # LargeEngine: twin nozzles at the block's bottom edge (no protrude, multi-cell)
    r=R(2,1); W,H=r.WU_w,r.WU_h; cx=W/2
    fill_block(r,6,6,W-6,H-6,"e2",haz_bottom=False)
    for cxf in (0.30,0.70):
        ex=W*cxf
        sphere_shade(r,ex,H*0.42,12,mix(STEEL_D,STEEL,0.35))
        for a in range(0,360,45): r.d.line([P(r,ex,H*0.42),(U(r,ex+math.cos(math.radians(a))*9),U(r,H*0.42+math.sin(math.radians(a))*9))],fill=A(STEEL,180),width=SS)
        hazard_band(r,ex-16,H-24,ex+16,H-19)
        r.d.polygon([P(r,ex-16,H-19),P(r,ex+16,H-19),P(r,ex+9,H-7),P(r,ex-9,H-7)],fill=mix(STEEL_D,DARK,0.3)+(255,),outline=EDGE+(255,))
        soot(r,ex,H-6,16,8)
        g,gd=layer(r); gd.polygon([P(r,ex-7,H-8),P(r,ex+7,H-8),P(r,ex,H+6)],fill=A(ORGL,220)); compose(r,blur(g,1.2))
    save(r,fn)

# LIFE SUPPORT
def oxygen(cw,ch,fn):
    r=R(cw,1); W,H=r.WU_w,r.WU_h
    fill_block(r,6,6,W-6,H-6,"o2")
    n=2 if cw<2 else 4
    for i in range(n):
        cxi=W*(i+0.5)/n
        cyl(r,cxi,18,H-24,18,mix(STEEL_D,STEEL,0.4),accent=GRN)
        led(r,cxi,16,GRN,1.6)
        r.d.ellipse([U(r,cxi-3),U(r,H*0.45-3),U(r,cxi+3),U(r,H*0.45+3)],outline=A(GRN,180),width=SS)
    rr(r,[W/2-16,H-22,W/2+16,H-8],radius=U(r,3),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
    r.d.ellipse([U(r,W/2-8),U(r,H-20),U(r,W/2+4),U(r,H-10)],fill=DARK+(255,),outline=EDGE+(200,),width=SS); led(r,W/2+9,H-18,GRN,1.3)
    save(r,fn)

def life_support(cw,ch,fn):
    r=R(1,1); W=r.WU_w
    fill_block(r,6,6,W-6,W-6,"ls")
    for i in range(3):
        cxi=W*(0.24+i*0.26)
        rr(r,[cxi-10,18,cxi+10,52],radius=U(r,3),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
        for gy in (26,32,38,44): r.d.line([P(r,cxi-8,gy),P(r,cxi+8,gy)],fill=A(GRN,90),width=SS)
        rr(r,[cxi-6,20,cxi+6,24],radius=U(r,1),fill=A(GRN,150))
    pipe(r,14,64,W-14,64,5); pipe(r,20,64,20,80,5)
    dscreen(r,[W*0.42,70,W*0.74,86],col=GRN)
    save(r,fn)

# WEAPONS (1x1 barrels protrude; multi-cell railgun at edge)
def point_defense(cw,ch,fn):
    # Base/mount only — the head + barrels are a separate rotating sprite
    # (turret_pd_barrel.png) the game spins to aim. No overhang here.
    r=R(1,1); W=r.WU_w; cx=cyb=W/2
    fill_block(r,6,6,W-6,W-6,"pd",haz_bottom=True)
    r.d.ellipse([U(r,cx-27),U(r,cyb-27),U(r,cx+27),U(r,cyb+27)],fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2); rim3d(r,cx,cyb,27,1.8)
    r.d.ellipse([U(r,cx-20),U(r,cyb-20),U(r,cx+20),U(r,cyb+20)],fill=mix(STEEL_D,DARK,0.35)+(255,),outline=EDGE+(255,),width=SS)  # recessed turret well
    led(r,cx-24,cyb,GRN,1.5); led(r,cx+24,cyb,RD,1.5)
    save(r,fn)

def railgun_1x1(cw,ch,fn):
    # Base/mount only — the rail barrel is a separate rotating sprite
    # (turret_rg_barrel.png). No overhang here.
    r=R(1,1); W=r.WU_w; cx=cyb=W/2
    fill_block(r,6,6,W-6,W-6,"rg",haz_bottom=True)
    for wx in (cx-24,cx+14): coilbank(r,wx,10,W-16,10)
    r.d.ellipse([U(r,cx-24),U(r,cyb-24),U(r,cx+24),U(r,cyb+24)],fill=mix(STEEL_D,STEEL,0.25)+(255,),outline=EDGE+(255,),width=SS*2); rim3d(r,cx,cyb,24,1.6)
    r.d.ellipse([U(r,cx-16),U(r,cyb-16),U(r,cx+16),U(r,cyb+16)],fill=mix(STEEL_D,DARK,0.35)+(255,),outline=EDGE+(255,),width=SS)  # recessed turret well
    save(r,fn)

def railgun_2x1(cw,ch,fn):
    r=R(2,1); W,H=r.WU_w,r.WU_h; cx=W/2
    fill_block(r,6,6,W-6,H-6,"rg2",haz_bottom=True)
    cyb=H*0.62
    for wx in (cx-58,cx+42): coilbank(r,wx,40,H-24,16)
    r.d.ellipse([U(r,cx-24),U(r,cyb-24),U(r,cx+24),U(r,cyb+24)],fill=mix(STEEL_D,STEEL,0.25)+(255,),outline=EDGE+(255,),width=SS*2); rim3d(r,cx,cyb,24,1.6)
    # long rail barrel toward the top edge
    for rxx in (cx-4,cx):
        r.d.rectangle([U(r,rxx-1),U(r,4),U(r,rxx+1),U(r,cyb)],fill=STEEL_L+(255,),outline=EDGE+(200,),width=SS)
    g,gd=layer(r); y=8
    while y<cyb-4: gd.rectangle([U(r,cx-4),U(r,y),U(r,cx+4),U(r,y+2)],fill=A(COREBLUE,220)); y+=10
    compose(r,blur(g,0.5)); small_glow(r,cx,cyb+8,8,COLD,HOT)
    save(r,fn)

def torpedo_tube(cw,ch,fn):
    r,by0,by1,ext=Rext2(26,'top'); W=r.WU_w; cx=W/2; BT=by0; BB=by1
    fill_block(r,6,BT,W-6,BB-4,"tt")
    for ox in (-11,11):
        rr(r,[cx+ox-7,BT+8,cx+ox+7,BB-12],radius=U(r,3),fill=mix(STEEL_D,DARK,0.25)+(255,),outline=EDGE+(255,),width=SS*2)
        # warhead pokes into extension
        r.d.rectangle([U(r,cx+ox-4),U(r,BT-ext+8),U(r,cx+ox+4),U(r,BT+16)],fill=mix(STEEL,STEEL_D,0.4)+(255,),outline=EDGE+(255,),width=SS)
        r.d.polygon([P(r,cx+ox-4,BT-ext+8),P(r,cx+ox+4,BT-ext+8),P(r,cx+ox,BT-ext+1)],fill=RD+(255,)); led(r,cx+ox,BT-ext+6,mix(RD,(255,255,255),0.4),1.0)
    hazard_band(r,cx-24,BB-11,cx+24,BB-6)
    save(r,fn)

def mine_layer(cw,ch,fn):
    r=R(1,1); W=r.WU_w
    fill_block(r,6,6,W-6,W-6,"ml")
    for i in range(3):
        cxi=W*(0.26+i*0.24)
        rr(r,[cxi-8,18,cxi+8,W-16],radius=U(r,3),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
        r.d.polygon([P(r,cxi-6,24),P(r,cxi+6,24),P(r,cxi,15)],fill=AMB+(255,))
        r.d.ellipse([U(r,cxi-4),U(r,W-34),U(r,cxi+4),U(r,W-26)],fill=mix(RD,DARK,0.2)+(255,),outline=EDGE+(200,),width=SS); led(r,cxi,W-30,RD,1.1)
    save(r,fn)

def salvage_arm(cw,ch,fn):
    r,by0,by1,ext=Rext2(34,'top'); W=r.WU_w; cx=W/2; BT=by0; BB=by1
    fill_block(r,6,BT,W-6,BB-4,"sa")
    baseY=BT+(BB-BT)*0.5
    r.d.ellipse([U(r,cx-14),U(r,baseY-8),U(r,cx+14),U(r,baseY+12)],fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2); rim3d(r,cx,baseY+2,13,1.4)
    j1=(cx,baseY-2); j2=(W*0.36,BT-ext*0.2); j3=(W*0.62,BT-ext*0.85)
    for a,b in [(j1,j2),(j2,j3)]:
        r.d.line([P(r,*a),P(r,*b)],fill=EDGE+(255,),width=SS*6); r.d.line([P(r,*a),P(r,*b)],fill=STEEL_L+(255,),width=SS*2)
    for j in (j1,j2,j3): r.d.ellipse([U(r,j[0]-4),U(r,j[1]-4),U(r,j[0]+4),U(r,j[1]+4)],fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
    cxx,cyy=j3
    for s in (-1,1):
        r.d.line([P(r,cxx,cyy),P(r,cxx+s*8,cyy-9)],fill=AMB+(255,),width=SS*2); r.d.line([P(r,cxx+s*8,cyy-9),P(r,cxx+s*11,cyy-5)],fill=AMB+(255,),width=SS*2)
    save(r,fn)

# SENSORS
def sonar_array(cw,ch,fn):
    r=R(1,1); W=r.WU_w; cx=W/2; cy=W*0.42
    fill_block(r,6,6,W-6,W-6,"son")
    rr(r,[cx-5,W*0.62,cx+5,W-14],radius=U(r,1.5),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
    for rad in (34,27,18,9):
        col=(6,14,18) if rad<12 else mix(STEEL,STEEL_D,(34-rad)/34)
        r.d.ellipse([U(r,cx-rad),U(r,cy-rad*0.75),U(r,cx+rad),U(r,cy+rad*0.75)],fill=col+(255,),outline=EDGE+(255,),width=SS)
    r.d.arc([U(r,cx-27),U(r,cy-20),U(r,cx+27),U(r,cy+20)],20,160,fill=A(STEEL_L,150),width=SS)
    r.d.line([P(r,cx,cy),P(r,cx,cy-30)],fill=STEEL_L+(255,),width=SS*2); small_glow(r,cx,cy-32,5,CY,(210,245,255))
    save(r,fn)

def passive_sonar(cw,ch,fn):
    r=R(cw,1); W,H=r.WU_w,r.WU_h
    fill_block(r,6,6,W-6,H-6,"ps")
    n=1 if cw<2 else 2
    for k in range(n):
        base=W*(k+0.5)/n
        for i in range(4):
            yy=20+i*13
            rr(r,[base-20,yy,base+20,yy+7],radius=U(r,2),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
            for j in range(5): led(r,base-16+j*8,yy+3.5,CY,1.1)
    dscreen(r,[W*0.32,H-24,W*0.68,H-10],col=CY)
    save(r,fn)

def depth_sensor(cw,ch,fn):
    r=R(1,1); W=r.WU_w; cx=W/2; cy=W*0.44
    fill_block(r,6,6,W-6,W-6,"ds")
    r.d.ellipse([U(r,cx-22),U(r,cy-22),U(r,cx+22),U(r,cy+22)],fill=(6,14,18,255),outline=EDGE+(255,),width=SS*2)
    for rad in (16,10,5): r.d.ellipse([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],outline=A(CY,90),width=SS)
    r.d.line([P(r,cx,cy),P(r,cx+15,cy-11)],fill=CY+(255,),width=SS*2)
    g,gd=layer(r); gd.pieslice([U(r,cx-16),U(r,cy-16),U(r,cx+16),U(r,cy+16)],-40,0,fill=A(CY,55)); compose(r,blur(g,0.6))
    led(r,cx+7,cy-13,GRN,1.2); led(r,cx-9,cy+7,GRN,1.0)
    for px in (16,W-26): rr(r,[px,W*0.72,px+10,W-14],radius=U(r,1.5),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
    save(r,fn)

# STORAGE
def cargo_hold(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h
    block_backing(r,H-4)
    nx=max(2,cw*2); ny=max(2,ch*2); rng=random.Random("cargo"+fn)
    pad=U(r,3); x0,y0,x1,y1=U(r,8),U(r,8),U(r,W-8),U(r,H-8)
    cwid=(x1-x0)/nx; chei=(y1-y0)/ny
    cols=[mix(STEEL_D,DARK,0.2),mix(AMB,STEEL_D,0.55),mix(STEEL,STEEL_D,0.4)]
    for iy in range(ny):
        for ix in range(nx):
            if (ix*7+iy*3)%5==4: continue
            bx0=x0+ix*cwid+pad; by0=y0+iy*chei+pad; bx1=x0+(ix+1)*cwid-pad; by1=y0+(iy+1)*chei-pad
            col=cols[(ix+iy)%3]
            r.d.rounded_rectangle([bx0,by0,bx1,by1],radius=U(r,1.5),fill=col+(255,),outline=EDGE+(255,),width=SS)
            r.d.line([(bx0+SS,by0+SS*2),(bx1-SS,by0+SS*2)],fill=A(STEEL_L,60),width=SS)
            r.d.line([((bx0+bx1)/2,by0+SS),((bx0+bx1)/2,by1-SS)],fill=A(EDGE,150),width=SS)
            r.d.line([(bx0+SS,(by0+by1)/2),(bx1-SS,(by0+by1)/2)],fill=A(AMB,60),width=SS)
    save(r,fn)

def ballast_tank(cw,ch,fn):
    r=R(1,1); W=r.WU_w; cx,cy=W/2,W/2
    fill_block(r,6,6,W-6,W-6,"ft")
    x0,x1,y0,y1=cx-19,cx+19,20,W-18
    cyl(r,cx,y0,y1,38,mix(STEEL,(90,80,62),0.35))
    hy0,hy1=cy-4,cy+5
    r.d.rectangle([U(r,x0+1),U(r,hy0),U(r,x1-1),U(r,hy1)],fill=mix(AMB,DARK,0.15)+(255,))
    step=U(r,5); sx=U(r,x0)-step; band=U(r,hy1-hy0)
    while sx<U(r,x1):
        r.d.polygon([(sx,U(r,hy1)),(sx+step*0.5,U(r,hy1)),(sx+step*0.5+band,U(r,hy0)),(sx+band,U(r,hy0))],fill=(20,18,14,255)); sx+=step
    tcx,tcy=cx,y0+(y1-y0)*0.16
    r.d.polygon([P(r,tcx,tcy-5),P(r,tcx-5,tcy+4),P(r,tcx+5,tcy+4)],fill=AMB+(255,),outline=EDGE+(255,))
    r.d.line([P(r,tcx,tcy-2),P(r,tcx,tcy+1)],fill=EDGE+(255,),width=SS)
    gy=y0+(y1-y0)*0.82; r.d.ellipse([U(r,cx-6),U(r,gy-6),U(r,cx+6),U(r,gy+6)],fill=(8,12,16,255),outline=EDGE+(255,),width=SS)
    r.d.line([P(r,cx,gy),P(r,cx+4,gy-3)],fill=AMB+(255,),width=SS)
    save(r,fn)

def research_lab(cw,ch,fn):
    r=R(cw,1); W,H=r.WU_w,r.WU_h; big=cw>=2
    fill_block(r,6,6,W-6,H-6,"rl")
    tcx=W*(0.26 if big else 0.33); tw=(11 if big else 9)*2
    cyl(r,tcx,18,H-18,tw,mix(STEEL_D,STEEL,0.3))
    r.d.rounded_rectangle([U(r,tcx-tw/2+2),U(r,22),U(r,tcx+tw/2-2),U(r,H-22)],radius=U(r,4),fill=(30,70,68,150))
    r.d.ellipse([U(r,tcx-6),U(r,H/2-6),U(r,tcx+6),U(r,H/2+6)],fill=(18,40,44,220))
    for a in range(4):
        ang=math.radians(20+a*36); r.d.line([P(r,tcx,H/2),(U(r,tcx+math.cos(ang)*8),U(r,H/2+math.sin(ang)*8))],fill=(18,40,44,200),width=SS*2)
    bx0=W*(0.50 if big else 0.52)
    rr(r,[bx0,H-30,W-16,H-16],radius=U(r,2),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
    for i,cc in enumerate([GRN,AMB,VIO]): r.d.rectangle([U(r,bx0+4+i*8),U(r,H-38),U(r,bx0+8+i*8),U(r,H-30)],fill=A(cc,200))
    dscreen(r,[bx0,22,W-16,H-40],col=TEALC)
    save(r,fn)

# COMMAND
def navigation(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h
    fill_block(r,6,6,W-6,H-6,"nav")
    if cw>=3:
        rr(r,[W*0.12,H*0.10,W*0.88,H*0.15],radius=U(r,3),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
        dscreen(r,[W*0.14,H*0.12,W*0.86,H*0.28],col=COREBLUE)
        for fx in (0.20,0.80):
            rr(r,[W*fx-14,H*0.50,W*fx+14,H*0.66],radius=U(r,3),fill=mix(STEEL_D,STEEL,0.2)+(255,),outline=EDGE+(255,),width=SS); dscreen(r,[W*fx-11,H*0.52,W*fx+11,H*0.62],col=CY)
            chair(r,W*fx,H*0.80)
        chair(r,W/2,H*0.62,big=True)
    elif cw>=2:
        for fx in (0.16,0.30):
            rr(r,[W*fx-8,H*0.20,W*fx+8,H*0.74],radius=U(r,2),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
            for gy in range(6): led(r,W*fx-4,H*0.26+gy*8,GRN,1.1); led(r,W*fx+4,H*0.27+gy*8,AMB,1.0)
        dscreen(r,[W*0.46,H*0.22,W*0.90,H*0.66],col=COREBLUE)
        chair(r,W*0.70,H*0.82)
    else:
        rr(r,[20,H*0.14,W-20,H*0.42],radius=U(r,3),fill=mix(STEEL_D,STEEL,0.2)+(255,),outline=EDGE+(255,),width=SS)
        dscreen(r,[26,H*0.18,W-26,H*0.36],col=CY)
        for i,c in enumerate([AMB,GRN,CY]): led(r,36+i*10,H*0.46,c,1.4)
        for px in (18,W-30): rr(r,[px,H*0.50,px+12,H*0.72],radius=U(r,2),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
        chair(r,W/2,H*0.74)
    save(r,fn)

# UTILITY
def repair_station(cw,ch,fn):
    r=R(cw,1); W,H=r.WU_w,r.WU_h; big=cw>=2
    fill_block(r,6,6,W-6,H-6,"rep")
    for i in range(5):
        tx=W*(0.14+i*0.11)
        rr(r,[tx-2,10,tx+2,14],radius=U(r,0.5),fill=([AMB,STEEL_L,AMB,STEEL_L,AMB][i])+(255,),outline=EDGE+(200,),width=SS)
    rr(r,[W*0.10,18,W*(0.60 if big else 0.90),30],radius=U(r,2),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
    rr(r,[W*0.12,H-30,W*0.36,H-14],radius=U(r,2),fill=mix(AMB,STEEL_D,0.5)+(255,),outline=EDGE+(255,),width=SS)
    if big:
        px,py=W*0.76,H/2+4
        r.d.ellipse([U(r,px-20),U(r,py-14),U(r,px+20),U(r,py+14)],outline=A(CY,110),width=SS)
        rr(r,[px-10,py-7,px+10,py+7],radius=U(r,3),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)
        for (ox,oy) in [(-13,-10),(13,-10),(-13,10),(13,10)]: r.d.ellipse([U(r,px+ox-4),U(r,py+oy-4),U(r,px+ox+4),U(r,py+oy+4)],fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
        small_glow(r,px,py,5,CY,(210,245,255))
    else:
        bx,by=W*0.70,H-18; j2=(W*0.62,H/2)
        r.d.line([P(r,bx,by),P(r,*j2)],fill=EDGE+(255,),width=SS*5); r.d.line([P(r,bx,by),P(r,*j2)],fill=STEEL_L+(255,),width=SS*2)
        r.d.line([P(r,*j2),P(r,W*0.80,H*0.34)],fill=EDGE+(255,),width=SS*4); r.d.line([P(r,*j2),P(r,W*0.80,H*0.34)],fill=STEEL_L+(255,),width=SS)
        r.d.ellipse([U(r,bx-8),U(r,by-5),U(r,bx+8),U(r,by+6)],fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS)
    save(r,fn)

def floodlight(cw,ch,fn):
    r=R(1,1); W=r.WU_w; cx=W/2
    fill_block(r,6,6,W-6,W-6,"fl")
    g,gd=layer(r)
    gd.polygon([P(r,cx-12,W*0.40),P(r,cx+12,W*0.40),P(r,cx+22,10),P(r,cx-22,10)],fill=(255,238,190,55))
    compose(r,blur(g,2.4))
    rr(r,[cx-16,W*0.40,cx+16,W*0.40+16],radius=U(r,3),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)
    for gy in range(int(W*0.40+3),int(W*0.40+15),3): r.d.line([P(r,cx+8,gy),P(r,cx+13,gy)],fill=A(EDGE,150),width=SS)
    r.d.ellipse([U(r,cx-11),U(r,W*0.40+3),U(r,cx+1),U(r,W*0.40+13)],fill=(255,244,200,255))
    r.d.ellipse([U(r,cx-8),U(r,W*0.40+5),U(r,cx-2),U(r,W*0.40+11)],fill=(255,252,235,255))
    rr(r,[cx-14,W-24,cx+14,W-12],radius=U(r,2),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS)  # base/fillers
    for bx in (cx-9,cx+7): bolt(r,bx,W-18,1.4)
    save(r,fn)

def docking_port(cw,ch,fn):
    r=R(cw,cw); W,H=r.WU_w,r.WU_h; cx,cy=W/2,H/2
    fill_block(r,6,6,W-6,H-6,"dock",haz_bottom=(cw>=3))
    rad=min(W,H)/2-14
    r.d.ellipse([U(r,cx-rad),U(r,cy-rad),U(r,cx+rad),U(r,cy+rad)],fill=mix(STEEL_D,STEEL,0.25)+(255,),outline=EDGE+(255,),width=SS*2); rim3d(r,cx,cy,rad,1.8)
    r.d.ellipse([U(r,cx-rad+5),U(r,cy-rad+5),U(r,cx+rad-5),U(r,cy+rad-5)],fill=mix(DARK,STEEL_D,0.5)+(255,),outline=EDGE+(255,),width=SS*2)
    for a in range(0,360,45):
        ex=cx+math.cos(math.radians(a))*rad*0.92; ey=cy+math.sin(math.radians(a))*rad*0.92
        ix=cx+math.cos(math.radians(a))*rad*0.62; iy=cy+math.sin(math.radians(a))*rad*0.62
        r.d.line([P(r,ix,iy),P(r,ex,ey)],fill=STEEL_L+(255,),width=SS*3)
    r.d.ellipse([U(r,cx-rad*0.5),U(r,cy-rad*0.5),U(r,cx+rad*0.5),U(r,cy+rad*0.5)],fill=mix(STEEL_D,STEEL,0.2)+(255,),outline=EDGE+(255,),width=SS*2)
    for a in range(0,360,60): r.d.line([P(r,cx,cy),(U(r,cx+math.cos(math.radians(a))*rad*0.5),U(r,cy+math.sin(math.radians(a))*rad*0.5))],fill=A(EDGE,180),width=SS)
    led(r,cx,cy,AMB,2.0)
    for a in range(0,360,90): led(r,cx+math.cos(math.radians(a))*(rad-3),cy+math.sin(math.radians(a))*(rad-3),GRN,1.2)
    save(r,fn)

# STRUCTURAL
def hull_beam(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h
    fill_block(r,5,5,W-5,H-5,"hull",base=mix(STEEL_D,DARK,0.28))
    # subtle plating seams per cell + extra rivets
    cwid=(W-10)/cw; chei=(H-10)/ch
    for i in range(1,cw): r.d.line([P(r,5+i*cwid,8),P(r,5+i*cwid,H-8)],fill=A(EDGE,180),width=SS*2)
    for j in range(1,ch): r.d.line([P(r,8,5+j*chei),P(r,W-8,5+j*chei)],fill=A(EDGE,180),width=SS*2)
    for i in range(cw):
        for j in range(ch):
            for (fx,fy) in [(0.2,0.2),(0.8,0.2),(0.2,0.8),(0.8,0.8)]:
                bolt(r,5+(i+fx)*cwid,5+(j+fy)*chei,1.5)
    save(r,fn)

# CREW ROOMS (dark)
def _dark_deck(r,x0,y0,x1,y1,tint):
    rr(r,[x0,y0,x1,y1],radius=U(r,5),fill=mix((40,42,50),tint,0.18)+(255,),outline=EDGE+(255,),width=SS*2)
    step=U(r,CELL); gx=U(r,x0)+step
    while gx<U(r,x1)-U(r,2): r.d.line([(gx,U(r,y0+3)),(gx,U(r,y1-3))],fill=A(DARK,120),width=SS); gx+=step
    gy=U(r,y0)+step
    while gy<U(r,y1)-U(r,2): r.d.line([(U(r,x0+3),gy),(U(r,x1-3),gy)],fill=A(DARK,120),width=SS); gy+=step

def basic_quarters(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h; F,G,u=r.fx,r.fy,r.u
    _dark_deck(r,5,5,W-5,H-5,(70,64,60))
    grime(r,8,8,W-8,H-8,"q"+fn)
    if cw>=2 or ch>=2:
        # bunk grid for barracks/galley/wellness — rows of bunks against walls
        rows=ch; colsn=cw
        for jj in range(max(1,rows)):
            for ii in range(max(1,colsn)):
                bx=F((ii+0.5)/colsn)-u(16); by=G(0.08)+jj*(H/max(1,rows))*0.0+G((jj+0.15)/max(1,rows))
                _bunk(r,F((ii+0.15)/colsn),G((jj+0.10)/max(1,rows)),u(30),u(46))
        _lamp_pt(r,F(0.5),G(0.5))
    else:
        _bunk(r,F(0.11),G(0.11),u(30),u(48))
        rr(r,[F(0.43),G(0.12),F(0.55),G(0.26)],radius=u(2),fill=WOOD+(255,),outline=EDGE+(255,),width=SS)  # nightstand
        _lamp_pt(r,F(0.49),G(0.19))
        rr(r,[F(0.78),G(0.13),F(0.90),G(0.46)],radius=u(2),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)  # wardrobe
        r.d.line([P(r,(0.84)*W,0.13*H+u(1)),P(r,0.84*W,0.46*H-u(1))],fill=A(EDGE,220),width=SS)
        rr(r,[F(0.56),G(0.80),F(0.89),G(0.90)],radius=u(2),fill=WOOD+(255,),outline=EDGE+(255,),width=SS*2)  # desk
        dscreen(r,[F(0.58)/1,G(0.81),F(0.70),G(0.88)],col=CY) if False else None
        rr(r,[F(0.66),G(0.66),F(0.78),G(0.78)],radius=u(3),fill=BLANK+(255,),outline=EDGE+(255,),width=SS)  # chair
        rr(r,[F(0.16),G(0.64),F(0.46),G(0.88)],radius=u(2),fill=(70,40,38)+(255,),outline=(48,28,26)+(255,),width=SS)  # rug
    save(r,fn)

def _bunk(r,x,y,w,h):
    u=r.u
    r.d.rounded_rectangle([x,y,x+w,y+h],radius=u(3),fill=WOOD+(255,),outline=EDGE+(255,),width=SS*2)
    r.d.rounded_rectangle([x,y,x+w,y+u(6)],radius=u(2),fill=mix(WOOD,DARK,0.3)+(255,))  # headboard
    r.d.rounded_rectangle([x+u(3),y+u(7),x+w-u(3),y+h-u(3)],radius=u(2),fill=LINEN+(255,))
    r.d.rounded_rectangle([x+u(4),y+u(8),x+w-u(4),y+u(6)+ (h*0.24)],radius=u(2),fill=(178,186,198)+(255,))  # pillow
    r.d.rounded_rectangle([x+u(3),y+h*0.44,x+w-u(3),y+h-u(3)],radius=u(2),fill=BLANK+(255,))

def _lamp_pt(r,cx,cy):
    g=Image.new("RGBA",r.img.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
    for rad,al in [(r.u(9),50),(r.u(5),110),(r.u(3),190)]: gd.ellipse([cx-rad,cy-rad,cx+rad,cy+rad],fill=(255,226,160,al))
    r.img.alpha_composite(blur(g,1.2)); r.d=ImageDraw.Draw(r.img)
    r.d.ellipse([cx-r.u(2),cy-r.u(2),cx+r.u(2),cy+r.u(2)],fill=(255,246,220,255))

def medical_bay(cw,ch,fn):
    r=R(cw,ch); W,H=r.WU_w,r.WU_h; F,G,u=r.fx,r.fy,r.u
    _dark_deck(r,5,5,W-5,H-5,(60,72,80))
    grime(r,8,8,W-8,H-8,"m"+fn)
    r.d.rectangle([U(r,8),U(r,H*0.60),U(r,W-8),U(r,H*0.60+u(2))],fill=A(RD,120))
    if cw>=3 and ch>=2:
        cx,cy=F(0.5),G(0.44)
        rr(r,[cx-u(14),cy-u(26),cx+u(14),cy+u(26)],radius=u(5),fill=mix(STEEL,STEEL_D,0.3)+(255,),outline=EDGE+(255,),width=SS*2)
        rr(r,[cx-u(11),cy-u(23),cx+u(11),cy+u(23)],radius=u(4),fill=(150,162,176)+(255,))
        r.d.ellipse([cx-u(6),cy-u(20),cx+u(6),cy-u(8)],fill=SKIN+(255,))
        for rad,al in [(u(20),50),(u(13),90),(u(7),150)]: r.d.ellipse([cx-rad,cy-rad,cx+rad,cy+rad],outline=(255,255,240,al),width=SS)
        _med_monitor(r,F(0.14),G(0.24)); _med_monitor(r,F(0.86),G(0.24))
        _med_cabinet(r,F(0.5)-u(10),G(0.78),u(20),u(16))
    else:
        _med_bed(r,F(0.11),G(0.10),F(0.42),G(0.52))
        _med_bed(r,F(0.58),G(0.10),F(0.89),G(0.52))
        _med_monitor(r,F(0.26),G(0.135)); _med_monitor(r,F(0.74),G(0.135))
        _med_cabinet(r,F(0.12),G(0.70),u(22),u(20))
        _med_crash(r,F(0.66),G(0.70),u(22),u(20))
    save(r,fn)

def _med_bed(r,x0,y0,x1,y1):
    u=r.u
    r.d.rounded_rectangle([x0,y0,x1,y1],radius=u(4),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)
    mx0,my0,mx1,my1=x0+u(4),y0+u(3),x1-u(4),y1-u(3)
    r.d.rounded_rectangle([mx0,my0,mx1,my1],radius=u(3),fill=(150,158,170)+(255,))
    r.d.rounded_rectangle([mx0+u(2),my0+u(2),mx1-u(2),my0+(my1-my0)*0.26],radius=u(3),fill=(178,186,198)+(255,))
    by=my0+(my1-my0)*0.42
    r.d.rounded_rectangle([mx0,by,mx1,my1],radius=u(3),fill=TEALC+(255,))
    hx=(mx0+mx1)/2; hy=my0+(my1-my0)*0.20; hrad=(mx1-mx0)*0.16
    r.d.ellipse([hx-hrad,hy-hrad,hx+hrad,hy+hrad],fill=SKIN+(255,))

def _med_monitor(r,cx,cy):
    u=r.u
    rr_=[cx-u(10),cy-u(7),cx+u(10),cy+u(7)]
    r.d.rounded_rectangle(rr_,radius=u(2),fill=mix(STEEL_D,DARK,0.2)+(255,),outline=EDGE+(255,),width=SS*2)
    r.d.rounded_rectangle([cx-u(8),cy-u(5),cx+u(8),cy+u(2)],radius=u(1),fill=(6,16,18)+(255,))
    r.d.line([(cx-u(6),cy-u(1)),(cx-u(2),cy-u(1)),(cx,cy-u(4)),(cx+u(2),cy+u(2)),(cx+u(6),cy-u(1))],fill=(120,220,150)+(255,),width=SS)
    led(r,cx-u(6),cy+u(5),GRN,1.1); led(r,cx,cy+u(5),AMB,1.1)

def _med_cabinet(r,x,y,w,h):
    u=r.u
    r.d.rounded_rectangle([x,y,x+w,y+h],radius=u(3),fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)
    cx,cy=x+w/2,y+h*0.5; s=min(w,h)*0.16
    r.d.rectangle([cx-s*0.33,cy-s,cx+s*0.33,cy+s],fill=RD+(255,)); r.d.rectangle([cx-s,cy-s*0.33,cx+s,cy+s*0.33],fill=RD+(255,))

def _med_crash(r,x,y,w,h):
    u=r.u
    r.d.rounded_rectangle([x,y,x+w,y+h],radius=u(3),fill=mix(RD,DARK,0.3)+(255,),outline=EDGE+(255,),width=SS*2)
    for i in range(3):
        dy=y+u(3)+i*(h-u(6))/3
        r.d.rounded_rectangle([x+u(2),dy,x+w-u(2),dy+(h-u(6))/3-u(2)],radius=u(1),fill=A(mix(RD,DARK,0.5),230))

# =====================================================================
DISPATCH={
 'small_reactor':lambda cw,ch,fn:_small_reactor() or None,   # cosmo dark reactor writes small_reactor.png
 'large_reactor':large_reactor,'battery':battery,
 'standard_engine':lambda cw,ch,fn:_thruster(),              # cosmo dark protruding engine
 'standard_engine_2x1':engine_2x1,'silent_drive':silent_drive,
 'oxygen_scrubber':oxygen,'life_support':life_support,
 'navigation':navigation,'point_defense':point_defense,
 'railgun':railgun_1x1,'railgun_2x1':railgun_2x1,'torpedo_tube':torpedo_tube,'mine_layer':mine_layer,
 'sonar_array':sonar_array,'passive_sonar':passive_sonar,'depth_sensor':depth_sensor,
 'cargo_hold':cargo_hold,'ballast_tank':ballast_tank,'research_lab':research_lab,
 'basic_quarters':basic_quarters,'medical_bay':medical_bay,
 'repair_station':repair_station,'floodlight':floodlight,
 'docking_port':docking_port,'salvage_arm':salvage_arm,'hull_beam':hull_beam,
}

def gen_one(base,cw,ch):
    fn=fname(base,cw,ch)
    key = base if (cw,ch)==(1,1) else fname(base,cw,ch).replace('.png','')
    # special multi-cell dispatch keys
    if base=='standard_engine' and (cw,ch)==(2,1): return engine_2x1(cw,ch,fn)
    if base=='railgun' and (cw,ch)==(2,1): return railgun_2x1(cw,ch,fn)
    fnc=DISPATCH.get(base)
    if fnc is None: print("  no builder for",base); return
    return fnc(cw,ch,fn)

if __name__=="__main__":
    made=[]
    for base,foots in JOBS.items():
        for (cw,ch) in foots:
            try:
                gen_one(base,cw,ch); made.append(fname(base,cw,ch))
            except Exception as e:
                import traceback; print("ERR",base,cw,ch,e); traceback.print_exc()
    print(f"generated {len(made)} sprites")
