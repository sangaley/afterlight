#!/usr/bin/env python3
"""Cosmoteer-icon detail level, pushed further. Chunky armored housings with
directional 3D shading, segmented armor rings, coolant pipe fittings, a
containment cage over a rich glowing core with plasma filaments, metal texture,
gimballed nozzle bells. Colors keep the game's language: BLUE power, ORANGE
thrust. No Cosmoteer assets used — style only. Exemplars: reactor + thruster."""
from smoothmach import *   # R, U, save, layer, compose, plate, palette, math ...
import math, random

# --- DARKER MOOD palette: grim, low-key, contained glows that pop on dark metal ---
STEEL=(80,88,102); STEEL_L=(148,160,180); STEEL_D=(48,54,66); EDGE=(10,12,17)
DARK=(6,8,12); LIGHT=(170,182,202)
COP=(158,104,52); COPL=(210,152,84); COPD=(96,58,32)
HOT=(206,234,255); COLD=(22,66,158); COREBLUE=(64,140,225)     # deep base, contained hot center
ORG=(222,118,42); ORGL=(240,176,104); ORGW=(255,238,204); ORGD=(130,52,16)
CAUT=(172,142,58)                                              # muted warning amber
GRIME=(26,28,36); RUST=(92,60,42); SOOT=(14,14,18)

def bolt(r,x,y,rad=2.0):
    d=r.d
    d.ellipse([U(r,x-rad),U(r,y-rad),U(r,x+rad),U(r,y+rad)],fill=mix(STEEL_D,EDGE,0.3)+(255,),outline=EDGE+(255,),width=SS)
    d.ellipse([U(r,x-rad*0.5),U(r,y-rad*0.55),U(r,x+rad*0.2),U(r,y+rad*0.1)],fill=A(STEEL_L,235))

def rim3d(r,cx,cy,rout,w=2.5,light=LIGHT,dark=DARK):
    box=[U(r,cx-rout),U(r,cy-rout),U(r,cx+rout),U(r,cy+rout)]
    r.d.arc(box,170,310,fill=A(light,210),width=int(SS*w))
    r.d.arc(box,-12,118,fill=A(dark,210),width=int(SS*w))

def sphere_shade(r,cx,cy,rout,base):
    d=r.d
    d.ellipse([U(r,cx-rout),U(r,cy-rout),U(r,cx+rout),U(r,cy+rout)],fill=base+(255,),outline=EDGE+(255,),width=SS*2)
    g,gd=layer(r)
    for i in range(6):
        rr=rout*(0.99-0.09*i); off=rout*0.18
        gd.ellipse([U(r,cx+off-rr),U(r,cy+off-rr),U(r,cx+off+rr),U(r,cy+off+rr)],fill=A(mix(base,DARK,0.62),52))
    for i in range(4):
        rr=rout*(0.66-0.11*i); off=rout*0.22
        gd.ellipse([U(r,cx-off-rr),U(r,cy-off-rr),U(r,cx-off+rr),U(r,cy-off+rr)],fill=A(mix(base,LIGHT,0.55),20))
    compose(r,blur(g,1.4))
    rim3d(r,cx,cy,rout)

def metal_noise(r,cx,cy,rout,seed):
    rng=random.Random(seed); g,gd=layer(r)
    for _ in range(260):
        a=rng.uniform(0,2*math.pi); rr=rng.uniform(0,rout-2)
        px=cx+math.cos(a)*rr; py=cy+math.sin(a)*rr
        if rng.random()<0.5: gd.point((U(r,px),U(r,py)),fill=(255,255,255,rng.choice([0,0,12,16])))
        else: gd.point((U(r,px),U(r,py)),fill=(0,0,0,rng.choice([0,14,18])))
    compose(r,blur(g,0.3))

def grime(r,x0,y0,x1,y1,seed):
    rng=random.Random(seed); g,gd=layer(r)
    for _ in range(7):
        sx=rng.uniform(x0+3,x1-3); hh=rng.uniform((y1-y0)*0.25,(y1-y0)*0.62); sy=rng.uniform(y0,y1-hh)
        gd.line([(U(r,sx),U(r,sy)),(U(r,sx),U(r,sy+hh))],fill=A(GRIME,rng.choice([28,42,56])),width=SS)
    for _ in range(12):
        sx=rng.uniform(x0+2,x1-2); sy=rng.uniform(y0+2,y1-2); rr=rng.uniform(1.5,4.0)
        col=rng.choice([GRIME,GRIME,GRIME,RUST])
        gd.ellipse([U(r,sx-rr),U(r,sy-rr),U(r,sx+rr),U(r,sy+rr)],fill=A(col,rng.choice([20,32])))
    compose(r,blur(g,0.5))

def soot(r,cx,cy,rw,rh):
    g,gd=layer(r)
    for i in range(7):
        w=rw*(1-0.11*i); h=rh*(1-0.11*i)
        gd.ellipse([U(r,cx-w),U(r,cy-h),U(r,cx+w),U(r,cy+h)],fill=A(SOOT,26))
    compose(r,blur(g,2.2))

def core(r,cx,cy,rad):
    # contained luminescence: bright hot centre, deep blue-black falloff so the
    # glow reads against dark metal instead of washing the whole part.
    g,gd=layer(r); N=34; edge=mix(COLD,DARK,0.45)
    for i in range(N,0,-1):
        t=i/N; rr=rad*t; c=mix(HOT,edge,t)
        gd.ellipse([U(r,cx-rr),U(r,cy-rr),U(r,cx+rr),U(r,cy+rr)],fill=c+(int(28+128*(1-t)),))
    gd.ellipse([U(r,cx-rad*0.20),U(r,cy-rad*0.20),U(r,cx+rad*0.20),U(r,cy+rad*0.20)],fill=(255,255,255,255))
    compose(r,blur(g,1.6))

def bevel_rect(r,x0,y0,x1,y1,base,rad=6):
    d=r.d
    d.rounded_rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],radius=U(r,rad),fill=base+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(U(r,x0+rad),U(r,y0+2)),(U(r,x1-rad),U(r,y0+2))],fill=A(LIGHT,150),width=SS)
    d.line([(U(r,x0+2),U(r,y0+rad)),(U(r,x0+2),U(r,y1-rad))],fill=A(LIGHT,110),width=SS)
    d.line([(U(r,x0+rad),U(r,y1-2)),(U(r,x1-rad),U(r,y1-2))],fill=A(DARK,150),width=SS)
    d.line([(U(r,x1-2),U(r,y0+rad)),(U(r,x1-2),U(r,y1-rad))],fill=A(DARK,130),width=SS)

def hazard_band(r,x0,y0,x1,y1):
    d=r.d
    d.rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],fill=CAUT+(255,))
    step=U(r,5); sx=U(r,x0)-step; band=U(r,y1-y0)
    while sx<U(r,x1):
        d.polygon([(sx,U(r,y1)),(sx+step*0.5,U(r,y1)),(sx+step*0.5+band,U(r,y0)),(sx+band,U(r,y0))],fill=(34,30,24,255)); sx+=step
    d.rectangle([U(r,x0),U(r,y0),U(r,x1),U(r,y1)],outline=EDGE+(255,),width=SS)

# ============================================================ REACTOR
def reactor():
    r=R(1,1); W=r.WU_w; cx=cy=W/2; d=r.d
    rout=W/2-6
    sphere_shade(r,cx,cy,rout,mix(STEEL,STEEL_D,0.55))
    metal_noise(r,cx,cy,rout,"reactor"); grime(r,cx-rout,cy-rout,cx+rout,cy+rout,"rgrime"); d=r.d
    # segmented armor ring: radial dividers + per-segment rivets + inner bevel
    rmid=rout-9
    for a in range(0,360,45):
        d.line([(U(r,cx+math.cos(math.radians(a))*rmid),U(r,cy+math.sin(math.radians(a))*rmid)),
                (U(r,cx+math.cos(math.radians(a))*rout),U(r,cy+math.sin(math.radians(a))*rout))],fill=A(EDGE,220),width=SS)
        mid=math.radians(a+22.5); bolt(r,cx+math.cos(mid)*(rout-4),cy+math.sin(mid)*(rout-4),1.7)
    d.ellipse([U(r,cx-rmid),U(r,cy-rmid),U(r,cx+rmid),U(r,cy+rmid)],outline=A(LIGHT,90),width=SS)
    # coolant pipe fittings at N/E/S/W
    for a in (270,0,90,180):
        ox,oy=math.cos(math.radians(a)),math.sin(math.radians(a))
        fx,fy=cx+ox*(rout-1),cy+oy*(rout-1)
        d.rounded_rectangle([U(r,fx-3),U(r,fy-3),U(r,fx+3),U(r,fy+3)],radius=U(r,1),fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)
        d.ellipse([U(r,fx-1.4),U(r,fy-1.4),U(r,fx+1.4),U(r,fy+1.4)],fill=DARK+(255,))
    # recessed containment ring
    rin=rmid-4
    d.ellipse([U(r,cx-rin),U(r,cy-rin),U(r,cx+rin),U(r,cy+rin)],fill=mix(STEEL_D,EDGE,0.4)+(255,),outline=EDGE+(255,),width=SS*2)
    rim3d(r,cx,cy,rin,w=1.6)
    # copper coil segments
    rc=rin-3
    for a0 in range(0,360,30):
        box=[U(r,cx-rc),U(r,cy-rc),U(r,cx+rc),U(r,cy+rc)]
        d.arc(box,a0+5,a0+25,fill=COP+(255,),width=SS*4)
        d.arc(box,a0+5,a0+25,fill=A(COPL,230),width=SS)
    # core well
    rcore=rc-5
    d.ellipse([U(r,cx-rcore),U(r,cy-rcore),U(r,cx+rcore),U(r,cy+rcore)],fill=mix(DARK,COLD,0.3)+(255,),outline=EDGE+(255,),width=SS)
    core(r,cx,cy,rcore-1); d=r.d
    # plasma filaments (curved bright licks)
    g,gd=layer(r)
    for k in range(3):
        a0=math.radians(k*120+15); pts=[]
        for t in range(0,7):
            ang=a0+t*0.55; rr=rcore*(0.14+0.11*t)
            pts.append((U(r,cx+math.cos(ang)*rr),U(r,cy+math.sin(ang)*rr)))
        gd.line(pts,fill=(212,236,255,220),width=SS)
    compose(r,blur(g,0.5)); d=r.d
    # containment cage: 3 tapered struts hub->ring, OVER the glow
    hub=6
    for a in (90,210,330):
        ox,oy=math.cos(math.radians(a)),math.sin(math.radians(a)); perp=(-oy,ox)
        p_in=(cx+ox*hub,cy+oy*hub); p_out=(cx+ox*rcore,cy+oy*rcore); w1,w2=3.2,1.6
        quad=[(p_in[0]+perp[0]*w1,p_in[1]+perp[1]*w1),(p_in[0]-perp[0]*w1,p_in[1]-perp[1]*w1),
              (p_out[0]-perp[0]*w2,p_out[1]-perp[1]*w2),(p_out[0]+perp[0]*w2,p_out[1]+perp[1]*w2)]
        d.polygon([(U(r,x),U(r,y)) for x,y in quad],fill=mix(STEEL,STEEL_D,0.3)+(255,),outline=EDGE+(255,))
        d.line([(U(r,p_in[0]+perp[0]*w1),U(r,p_in[1]+perp[1]*w1)),(U(r,p_out[0]+perp[0]*w2),U(r,p_out[1]+perp[1]*w2))],fill=A(LIGHT,150),width=SS)
    # central hub (glowing)
    d.ellipse([U(r,cx-hub),U(r,cy-hub),U(r,cx+hub),U(r,cy+hub)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)
    d.ellipse([U(r,cx-hub+1),U(r,cy-hub+1),U(r,cx+hub-2),U(r,cy+hub-2)],fill=A(HOT,210))
    d.ellipse([U(r,cx-2),U(r,cy-2),U(r,cx+2),U(r,cy+2)],fill=(255,255,255,255))
    # status LEDs low-left / low-right on the ring
    for a,c in [(118,(120,225,150,255)),(62,(232,190,96,255))]:
        lx=cx+math.cos(math.radians(a))*(rmid-2); ly=cy+math.sin(math.radians(a))*(rmid-2)
        d.ellipse([U(r,lx-1.6),U(r,ly-1.6),U(r,lx+1.6),U(r,ly+1.6)],fill=c)
    save(r,"small_reactor.png")

# ============================================================ THRUSTER (exhaust DOWN, ORANGE)
def thruster():
    r=R(1,1); W,Hh=r.WU_w,r.WU_h; cx=W/2; d=r.d
    # fuel manifold + injectors
    d.rounded_rectangle([U(r,cx-24),U(r,6),U(r,cx+24),U(r,12)],radius=U(r,2),fill=mix(STEEL_D,STEEL,0.5)+(255,),outline=EDGE+(255,),width=SS)
    d.line([(U(r,cx-22),U(r,7)),(U(r,cx+22),U(r,7))],fill=A(LIGHT,150),width=SS)
    for ox in (-14,0,14):
        d.rectangle([U(r,cx+ox-1.5),U(r,12),U(r,cx+ox+1.5),U(r,15)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,180),width=SS)
    # fuel lines down the sides with flanges
    for side in (-1,1):
        lx=cx+side*22
        d.line([(U(r,cx+side*20),U(r,9)),(U(r,lx),U(r,18)),(U(r,lx),U(r,44))],fill=EDGE+(255,),width=SS*3)
        d.line([(U(r,cx+side*20),U(r,9)),(U(r,lx),U(r,18)),(U(r,lx),U(r,44))],fill=COP+(255,),width=SS)
        for fy in (24,36): d.rectangle([U(r,lx-2),U(r,fy-1),U(r,lx+2),U(r,fy+1)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,180),width=SS)
    # main armored housing (beveled 3D) + noise
    bevel_rect(r,cx-26,15,cx+26,54,mix(STEEL,STEEL_D,0.15),rad=6)
    metal_noise(r,cx,34,26,"thruster"); d=r.d
    for gy in (24,44): d.line([(U(r,cx-22),U(r,gy)),(U(r,cx+22),U(r,gy))],fill=A(EDGE,110),width=SS)
    for (bx,by) in [(cx-22,19),(cx+22,19),(cx-22,50),(cx+22,50)]: bolt(r,bx,by,1.6)
    # central turbopump (shaded metal disc + blades + hub)
    sphere_shade(r,cx,32,10,mix(STEEL_D,STEEL,0.4)); d=r.d
    for a in range(0,360,40):
        d.line([(U(r,cx),U(r,32)),(U(r,cx+math.cos(math.radians(a))*8),U(r,32+math.sin(math.radians(a))*8))],fill=A(STEEL,200),width=SS)
    d.ellipse([U(r,cx-3),U(r,29),U(r,cx+3),U(r,35)],fill=STEEL_L+(255,),outline=EDGE+(200,),width=SS)
    # combustion band (warning)
    hazard_band(r,cx-24,55,cx+24,60)
    # gimbal ring where bell meets housing
    d.ellipse([U(r,cx-22),U(r,58),U(r,cx+22),U(r,64)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)
    d.arc([U(r,cx-22),U(r,58),U(r,cx+22),U(r,64)],170,360,fill=A(LIGHT,150),width=SS)
    # nozzle bell — flared, ribbed, inner shadow, rim light
    bt,bb=61,82
    def P(x,y): return (U(r,x),U(r,y))
    d.polygon([P(cx-24,bt),P(cx+24,bt),P(cx+15,bb),P(cx-15,bb)],fill=mix(STEEL_D,EDGE,0.2)+(255,),outline=EDGE+(255,))
    d.polygon([P(cx-18,bt+2),P(cx+18,bt+2),P(cx+12,bb-1),P(cx-12,bb-1)],fill=mix(DARK,STEEL_D,0.4)+(255,))
    for t in (0.2,0.4,0.6,0.8):
        xt=cx-24+48*t; xb=cx-15+30*t
        d.line([P(xt,bt+1),P(xb,bb-1)],fill=A(EDGE,150),width=SS)
    d.line([P(cx-23,bt+1),P(cx-14,bb-1)],fill=A(LIGHT,140),width=SS)
    # bright throat + hot exhaust bloom
    g,gd=layer(r)
    gd.ellipse([U(r,cx-9),U(r,bt+1),U(r,cx+9),U(r,bt+7)],fill=A(ORGW,220))
    gd.ellipse([U(r,cx-16),U(r,bb-4),U(r,cx+16),U(r,bb+6)],fill=A(ORGL,150))
    gd.polygon([P(cx-14,bb),P(cx+14,bb),P(cx+7,bb+18),P(cx-7,bb+18)],fill=A(ORG,200))
    gd.polygon([P(cx-8,bb),P(cx+8,bb),P(cx,bb+22)],fill=A(ORGL,235))
    gd.polygon([P(cx-4,bb),P(cx+4,bb),P(cx,bb+14)],fill=A(ORGW,255))
    compose(r,blur(g,1.3)); d=r.d
    # status LEDs
    d.ellipse([U(r,cx-20),U(r,40),U(r,cx-17),U(r,43)],fill=(120,225,150,255))
    d.ellipse([U(r,cx+17),U(r,40),U(r,cx+20),U(r,43)],fill=(232,96,74,255))
    save(r,"standard_engine.png")

# ============================================================ THRUSTER w/ OVERHANG
# The nozzle protrudes PAST the block. Canvas is taller than the cell; the block
# region is the top square (gray-backed), the nozzle + flame live in the bottom
# extension and stick out. Extension in DISPLAY units = 30 (matches spawner);
# art units = 30 * WU_w/local_w = 30 * 126/60 = 63 for a 1x1.
def Rext(ext_disp=30.0):
    r=Room(1,1); r.img=Image.new("RGBA",(r.W,r.H),(0,0,0,0))
    art_ext=ext_disp*(r.WU_w/60.0)          # 1x1: local_w=60
    r.WU_h=r.WU_h+art_ext
    r.H=int(r.WU_h*FINAL*SS)
    r.img=Image.new("RGBA",(r.W,r.H),(0,0,0,0)); r.d=ImageDraw.Draw(r.img)
    return r, 126.0                          # block edge (BE) in art units = original WU_h

def block_backing(r, y1):
    d=r.d; GRAY=(60,64,74)
    b=[U(r,4),U(r,4),U(r,r.WU_w-4),U(r,y1)]
    d.rounded_rectangle(b,radius=U(r,7),fill=GRAY+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([(b[0]+U(r,5),b[1]+SS*2),(b[2]-U(r,5),b[1]+SS*2)],fill=A(mix(GRAY,(255,255,255),0.28),160),width=SS)
    for (rx,ry) in [(9,9),(r.WU_w-9,9),(9,y1-5),(r.WU_w-9,y1-5)]:
        d.ellipse([U(r,rx-1.8),U(r,ry-1.8),U(r,rx+1.8),U(r,ry+1.8)],fill=mix(GRAY,EDGE,0.5)+(255,))
        d.ellipse([U(r,rx-1.1),U(r,ry-1.1),U(r,rx+0.3),U(r,ry+0.3)],fill=A(mix(GRAY,(255,255,255),0.4),220))

def thruster_overhang():
    r,BE=Rext(30.0); W=r.WU_w; cx=W/2; d=r.d
    def P(x,y): return (U(r,x),U(r,y))
    block_backing(r, BE-4)
    # --- FULL-BLOCK armored housing (fills the whole cell, edge to edge) ---
    bevel_rect(r, 6, 6, W-6, BE-5, mix(STEEL,STEEL_D,0.35), rad=8)
    metal_noise(r,cx,BE/2,W/2-6,"thruster"); grime(r,8,8,W-8,BE-8,"tgrime"); d=r.d
    # top intake manifold spanning the full width + injectors
    d.rounded_rectangle([U(r,12),U(r,10),U(r,W-12),U(r,18)],radius=U(r,2),fill=mix(STEEL_D,STEEL,0.5)+(255,),outline=EDGE+(255,),width=SS)
    d.line([(U(r,14),U(r,11)),(U(r,W-14),U(r,11))],fill=A(LIGHT,160),width=SS)
    for ox in range(-40,41,16):
        d.rectangle([U(r,cx+ox-1.5),U(r,18),U(r,cx+ox+1.5),U(r,21)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,180),width=SS)
    # side FILLERS: coolant vent grilles + vertical copper pipe runs
    for side in (-1,1):
        for gy in range(30,int(BE-26),6):
            d.line([P(cx+side*32,gy),P(cx+side*50,gy)],fill=A(EDGE,150),width=SS)
        d.line([P(cx+side*54,24),P(cx+side*54,BE-22)],fill=EDGE+(255,),width=SS*3)
        d.line([P(cx+side*54,24),P(cx+side*54,BE-22)],fill=COP+(255,),width=SS)
        for fy in (34,BE-34): d.rectangle([U(r,cx+side*54-2.5),U(r,fy-1.5),U(r,cx+side*54+2.5),U(r,fy+1.5)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=A(EDGE,180),width=SS)
    # bolt rows down both edges + across the top
    for bx in (12,W-12):
        for by in (16,BE*0.34,BE*0.60,BE-16): bolt(r,bx,by,1.7)
    # panel seam splitting upper/lower housing
    d.line([P(14,BE*0.52),P(W-14,BE*0.52)],fill=A(EDGE,120),width=SS)
    d.line([P(14,BE*0.52+1),P(W-14,BE*0.52+1)],fill=A(LIGHT,50),width=SS)
    # central turbopump (large)
    tpy=BE*0.40
    sphere_shade(r,cx,tpy,16,mix(STEEL_D,STEEL,0.4)); d=r.d
    for a in range(0,360,30):
        d.line([P(cx,tpy),(U(r,cx+math.cos(math.radians(a))*13),U(r,tpy+math.sin(math.radians(a))*13))],fill=A(STEEL,200),width=SS)
    d.ellipse([U(r,cx-4),U(r,tpy-4),U(r,cx+4),U(r,tpy+4)],fill=STEEL_L+(255,),outline=EDGE+(200,),width=SS)
    # combustion band + gimbal ring at the block's lower edge (full width)
    hazard_band(r,cx-46,BE-16,cx+46,BE-11)
    d.ellipse([U(r,cx-32),U(r,BE-9),U(r,cx+32),U(r,BE-1)],fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)
    d.arc([U(r,cx-32),U(r,BE-9),U(r,cx+32),U(r,BE-1)],170,360,fill=A(LIGHT,150),width=SS)
    soot(r,cx,BE+10,40,22); d=r.d      # exhaust scorch around the wide nozzle exit
    # --- NOZZLE BELL: flares OPEN past the block edge (throat narrow, mouth wide) ---
    bt,bb=BE-5, BE+40
    d.polygon([P(cx-22,bt),P(cx+22,bt),P(cx+35,bb),P(cx-35,bb)],fill=mix(STEEL_D,EDGE,0.2)+(255,),outline=EDGE+(255,))
    # wide open dark muzzle interior + inner rim for depth
    d.polygon([P(cx-16,bt+2),P(cx+16,bt+2),P(cx+29,bb-1),P(cx-29,bb-1)],fill=DARK+(255,))
    d.polygon([P(cx-11,bt+4),P(cx+11,bt+4),P(cx+22,bb-3),P(cx-22,bb-3)],fill=mix(DARK,STEEL_D,0.55)+(255,))
    for t in (0.25,0.5,0.75):
        xt=cx-22+44*t; xb=cx-35+70*t
        d.line([P(xt,bt+1),P(xb,bb-1)],fill=A(EDGE,150),width=SS)
    d.line([P(cx-21,bt+1),P(cx-34,bb-1)],fill=A(LIGHT,140),width=SS)   # left rim light
    d.line([P(cx+21,bt+1),P(cx+34,bb-1)],fill=A(EDGE,190),width=SS)    # right rim shade
    # exhaust venting from the wide open mouth
    g,gd=layer(r)
    gd.ellipse([U(r,cx-29),U(r,bb-7),U(r,cx+29),U(r,bb+9)],fill=A(ORGL,140))
    gd.polygon([P(cx-27,bb),P(cx+27,bb),P(cx+13,bb+28),P(cx-13,bb+28)],fill=A(ORG,200))
    gd.polygon([P(cx-15,bb),P(cx+15,bb),P(cx,bb+34)],fill=A(ORGL,235))
    gd.polygon([P(cx-7,bb),P(cx+7,bb),P(cx,bb+24)],fill=A(ORGW,255))
    compose(r,blur(g,1.4)); d=r.d
    # status LEDs on housing
    d.ellipse([U(r,20),U(r,BE*0.34),U(r,23),U(r,BE*0.34+3)],fill=(120,225,150,255))
    d.ellipse([U(r,W-23),U(r,BE*0.34),U(r,W-20),U(r,BE*0.34+3)],fill=(232,96,74,255))
    save(r,"standard_engine.png")

if __name__=="__main__":
    reactor(); thruster_overhang()
    print("cosmo pushed exemplars done (thruster overhangs block)")
