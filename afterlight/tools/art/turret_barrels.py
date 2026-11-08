#!/usr/bin/env python3
"""Pivot-centred rotating barrel sprites for gun turrets (point-defense + rail).
Drawn on a square canvas with the turret pivot at the CENTER and the barrel(s)
pointing UP (+Y / toward the top). The in-game turret system rotates these about
their center to aim. Dark-pixel style to match the modules."""
from PIL import Image, ImageDraw
import os
SS=4
DEST="/Users/shhh/afterlight/afterlight/assets/sprites/modules"

EDGE=(11,13,18); DARK=(7,9,13)
STEEL=(80,88,102); STEEL_L=(150,162,180); STEEL_D=(50,56,68)
COP=(158,104,52); COPL=(210,152,84); COREBLUE=(80,160,245); HOT=(206,234,255)
GRN=(120,232,150); RD=(206,74,60); AMB=(212,168,84)

def A(c,a): return (c[0],c[1],c[2],a)
def mix(a,b,t): return tuple(int(a[i]*(1-t)+b[i]*t) for i in range(3))
def step_alpha(a): return a.point(lambda v: min(255,int(round(v/51))*51))

def new(N):
    im=Image.new("RGBA",(N*SS,N*SS),(0,0,0,0)); return im, ImageDraw.Draw(im)
def finish(im,name,N,colors=20):
    down=im.resize((N,N),Image.BOX)
    rgb=down.convert("RGB").quantize(colors=colors,dither=Image.Dither.NONE).convert("RGB")
    al=step_alpha(down.getchannel("A")); out=rgb.convert("RGBA"); out.putalpha(al)
    out.save(f"{DEST}/{name}"); print("·",name,out.size)

def R(v): return int(v*SS)
def box(d,x0,y0,x1,y1,rad=0,**k): d.rounded_rectangle([R(x0),R(y0),R(x1),R(y1)],radius=R(rad),**k)
def ell(d,x0,y0,x1,y1,**k): d.ellipse([R(x0),R(y0),R(x1),R(y1)],**k)

# ---- point-defense: twin autocannon barrels + turret head ----
def pd_barrel():
    N=264; im,d=new(N); c=N//2  # pivot at center
    # rotating turret head housing over the pivot
    ell(d,c-40,c-40,c+40,c+40,fill=mix(STEEL_D,STEEL,0.35)+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([R(c-40),R(c-40),R(c+40),R(c+40)],200,340,fill=A(STEEL_L,150),width=SS)
    box(d,c-30,c-16,c+30,c+34,rad=8,fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)  # gun mantlet
    d.line([(R(c-26),R(c-12)),(R(c+26),R(c-12))],fill=A(STEEL_L,140),width=SS)
    # twin barrels up (toward top)
    for ox in (-14,14):
        box(d,c+ox-6,c-108,c+ox+6,c-4,rad=3,fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
        d.line([(R(c+ox-3),R(c-104)),(R(c+ox-3),R(c-8))],fill=A((235,242,250),200),width=SS)  # highlight
        box(d,c+ox-9,c-112,c+ox+9,c-100,rad=2,fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)  # muzzle brake
        box(d,c+ox-4,c-112,c+ox+4,c-108,fill=DARK+(255,))  # bore
    # status lamps on the head
    ell(d,c-34,c+2,c-30,c+6,fill=GRN+(255,)); ell(d,c+30,c+2,c+34,c+6,fill=RD+(255,))
    finish(im,"turret_pd_barrel.png",N)

# ---- railgun: single long rail barrel + head ----
def rg_barrel():
    N=300; im,d=new(N); c=N//2
    ell(d,c-38,c-38,c+38,c+38,fill=mix(STEEL_D,STEEL,0.35)+(255,),outline=EDGE+(255,),width=SS*2)
    d.arc([R(c-38),R(c-38),R(c+38),R(c+38)],200,340,fill=A(STEEL_L,150),width=SS)
    box(d,c-24,c-14,c+24,c+30,rad=6,fill=mix(STEEL_D,STEEL,0.3)+(255,),outline=EDGE+(255,),width=SS*2)  # breech
    # twin rails up with energized segments
    for rx in (c-7,c+3):
        box(d,rx-2,c-134,rx+2,c-4,rad=1,fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
    g=Image.new("RGBA",im.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
    y=c-128
    while y<c-8:
        gd.rectangle([R(c-7),R(y),R(c+7),R(y+3)],fill=A(COREBLUE,230)); y+=11
    from PIL import ImageFilter
    im.alpha_composite(g.filter(ImageFilter.GaussianBlur(SS*0.5))); d=ImageDraw.Draw(im)
    box(d,c-11,c-138,c+11,c-128,rad=2,fill=mix(STEEL_D,STEEL,0.4)+(255,),outline=EDGE+(255,),width=SS)  # muzzle
    box(d,c-4,c-138,c+4,c-132,fill=DARK+(255,))
    # capacitor glow at breech
    g2=Image.new("RGBA",im.size,(0,0,0,0)); gd2=ImageDraw.Draw(g2)
    for rr,al in [(14,60),(9,120),(5,200)]:
        gd2.ellipse([R(c-rr),R(c+10-rr),R(c+rr),R(c+10+rr)],fill=A(mix(COREBLUE,HOT,0.3),al))
    im.alpha_composite(g2.filter(ImageFilter.GaussianBlur(SS*0.6)))
    finish(im,"turret_rg_barrel.png",N)

if __name__=="__main__":
    pd_barrel(); rg_barrel(); print("done")
