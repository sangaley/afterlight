#!/usr/bin/env python3
"""Dark-pixel redesign of combat EFFECTS (kept bright — energy/fire) and
ENVIRONMENT objects (dark, grimy) to match the module art. Each drawn at
supersample, downscaled to its exact native size, palette-quantized + alpha-
stepped for the pixel look."""
from PIL import Image, ImageDraw, ImageFilter
import os, math, random

SS=5
OUT=os.path.join(os.path.dirname(__file__),"efx"); os.makedirs(OUT,exist_ok=True)
os.makedirs(OUT+"/effects",exist_ok=True); os.makedirs(OUT+"/environment",exist_ok=True)

EDGE=(12,14,20); DARK=(8,10,14)
ROCK=(62,64,74); ROCK_D=(38,40,50); ROCK_L=(98,100,112); ROCK_LL=(140,142,156)
MET=(74,80,92); MET_D=(44,48,58); MET_L=(126,134,148); RUST=(110,72,48)
WHITE=(255,255,255); YEL=(255,232,150); ORG=(250,150,58); ORGD=(200,90,36); RED=(216,60,50); REDD=(150,36,32)
CY=(118,220,240); CYD=(50,120,150); BLU=(96,172,255); BLUD=(40,90,170)
GRN=(120,232,150); GRND=(46,120,74); PURP=(176,124,232); PURPD=(90,60,140)
BIO=(128,236,158); BIOHI=(210,255,220)

def A(c,a): return (c[0],c[1],c[2],a)
def mix(a,b,t): return tuple(int(a[i]*(1-t)+b[i]*t) for i in range(3))

def canvas(w,h):
    im=Image.new("RGBA",(w*SS,h*SS),(0,0,0,0)); return im, ImageDraw.Draw(im)
def blur(im,r): return im.filter(ImageFilter.GaussianBlur(r*SS))
def step_alpha(a): return a.point(lambda v: min(255,int(round(v/51))*51))

def finish(im,cat,name,w,h,colors=24):
    down=im.resize((w,h),Image.BOX)
    rgb=down.convert("RGB").quantize(colors=colors,dither=Image.Dither.NONE).convert("RGB")
    al=step_alpha(down.getchannel("A"))
    out=rgb.convert("RGBA"); out.putalpha(al)
    out.save(f"{OUT}/{cat}/{name}"); print("·",cat,name,(w,h))

def S(v): return int(v*SS)

# glow disc helper (draw on a layer, composite blurred)
def glow(im, cx, cy, rad, cold, hot, alpha=200):
    g=Image.new("RGBA",im.size,(0,0,0,0)); gd=ImageDraw.Draw(g); N=14
    for i in range(N,0,-1):
        t=i/N; rr=rad*SS*t; c=mix(hot,cold,t)
        gd.ellipse([cx*SS-rr,cy*SS-rr,cx*SS+rr,cy*SS+rr],fill=c+(int(alpha*(1-t)*0.6+30),))
    gd.ellipse([cx*SS-rad*SS*0.25,cy*SS-rad*SS*0.25,cx*SS+rad*SS*0.25,cy*SS+rad*SS*0.25],fill=hot+(255,))
    im.alpha_composite(blur(g,0.6))

# ============================================================ EFFECTS (bright)
def explosion():
    w=h=48; im,d=canvas(w,h); cx,cy=w/2,h/2; rng=random.Random("expl")
    # fireball rings hot->cool
    for rad,col in [(22,REDD),(18,RED),(14,ORGD),(10,ORG),(6,YEL),(3,WHITE)]:
        # irregular blob
        pts=[]
        for a in range(0,360,20):
            rr=rad*(0.8+rng.random()*0.4)
            pts.append((S(cx+math.cos(math.radians(a))*rr),S(cy+math.sin(math.radians(a))*rr)))
        d.polygon(pts,fill=col+(255,))
    # sparks
    for _ in range(10):
        a=rng.random()*math.tau if hasattr(rng,'tau') else rng.random()*6.283; dist=rng.uniform(16,23)
        sx,sy=cx+math.cos(a)*dist,cy+math.sin(a)*dist
        d.rectangle([S(sx),S(sy),S(sx)+SS,S(sy)+SS],fill=YEL+(255,))
    finish(im,"effects","explosion.png",w,h,colors=16)

def enemy_projectile():
    w,h=16,8; im,d=canvas(w,h); cx,cy=w/2,h/2
    glow(im,cx,cy,4,REDD,YEL,alpha=180); d=ImageDraw.Draw(im)
    d.ellipse([S(3),S(2),S(13),S(6)],fill=RED+(255,))
    d.ellipse([S(9),S(2.5),S(14),S(5.5)],fill=YEL+(255,))   # hot front
    d.rectangle([S(12),S(3),S(15),S(5)],fill=WHITE+(255,))
    finish(im,"effects","enemy_projectile.png",w,h,colors=12)

def torpedo_trail():
    w,h=32,8; im,d=canvas(w,h)
    # fading smoke streak front(right)->back(left)
    for i in range(8):
        x=28-i*3.4; r=1.2+i*0.5; a=200-i*24
        col=mix(ORG,(90,90,100),i/8)
        d.ellipse([S(x-r),S(4-r),S(x+r),S(4+r)],fill=A(col,max(0,a)))
    d.ellipse([S(27),S(2.5),S(31),S(5.5)],fill=YEL+(255,))
    finish(im,"effects","torpedo_trail.png",w,h,colors=14)

def shield_bubble():
    w=h=256; im,d=canvas(w,h); cx,cy=w/2,h/2; R=120
    # soft fill
    g=Image.new("RGBA",im.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
    gd.ellipse([S(cx-R),S(cy-R),S(cx+R),S(cy+R)],fill=A(BLU,40))
    im.alpha_composite(blur(g,3)); d=ImageDraw.Draw(im)
    # hex facets
    for ring in range(3):
        rr=R*(0.5+ring*0.24)
        for a in range(0,360,30):
            hx=cx+math.cos(math.radians(a))*rr; hy=cy+math.sin(math.radians(a))*rr
            s=14
            pts=[(S(hx+math.cos(math.radians(k))*s),S(hy+math.sin(math.radians(k))*s)) for k in range(0,360,60)]
            d.polygon(pts,outline=A(CY,90),width=SS)
    # bright rim
    d.ellipse([S(cx-R),S(cy-R),S(cx+R),S(cy+R)],outline=A(CY,220),width=SS*3)
    d.ellipse([S(cx-R+4),S(cy-R+4),S(cx+R-4),S(cy+R-4)],outline=A(BLU,120),width=SS)
    finish(im,"effects","shield_bubble.png",w,h,colors=16)

def sonar_ring():
    w=h=64; im,d=canvas(w,h); cx,cy=w/2,h/2; R=28
    d.ellipse([S(cx-R),S(cy-R),S(cx+R),S(cy+R)],outline=A(GRN,230),width=SS*2)
    d.ellipse([S(cx-R+3),S(cy-R+3),S(cx+R-3),S(cy+R-3)],outline=A(GRN,90),width=SS)
    finish(im,"effects","sonar_ring.png",w,h,colors=10)

def electric_shock():
    w=h=32; im,d=canvas(w,h); cx,cy=w/2,h/2; rng=random.Random("elec")
    for b in range(5):
        a0=rng.random()*6.283; x,y=cx,cy; pts=[(S(x),S(y))]
        for seg in range(4):
            a0+=rng.uniform(-0.6,0.6); step=rng.uniform(3,5)
            x+=math.cos(a0)*step; y+=math.sin(a0)*step; pts.append((S(x),S(y)))
        d.line(pts,fill=A(CY,230),width=SS)
        d.line(pts,fill=A(WHITE,200),width=max(1,SS//2))
    glow(im,cx,cy,4,CYD,WHITE,alpha=140)
    finish(im,"effects","electric_shock.png",w,h,colors=10)

def bubble():
    w=h=16; im,d=canvas(w,h); cx,cy=w/2,h/2
    d.ellipse([S(3),S(3),S(13),S(13)],outline=A(CY,200),width=SS)
    d.ellipse([S(4),S(4),S(12),S(12)],fill=A(BLU,50))
    d.ellipse([S(5),S(4.5),S(7.5),S(7)],fill=A(WHITE,200))  # highlight
    finish(im,"effects","bubble.png",w,h,colors=8)

# ============================================================ ENVIRONMENT (dark)
def _rock_blob(d,cx,cy,rad,rng,base=ROCK):
    pts=[]
    for a in range(0,360,24):
        rr=rad*(0.72+rng.random()*0.5)
        pts.append((S(cx+math.cos(math.radians(a))*rr),S(cy+math.sin(math.radians(a))*rr)))
    d.polygon(pts,fill=base+(255,),outline=EDGE+(255,))
    # top-left light facet
    d.polygon(pts[:4]+[(S(cx),S(cy))],fill=mix(base,ROCK_LL,0.35)+(120,))
    return pts

def rock():
    w=h=48; im,d=canvas(w,h); rng=random.Random("rock")
    _rock_blob(d,24,24,20,rng)
    for _ in range(5):  # craters
        cx=rng.uniform(10,38); cy=rng.uniform(10,38); r=rng.uniform(2,5)
        d.ellipse([S(cx-r),S(cy-r),S(cx+r),S(cy+r)],fill=ROCK_D+(255,))
        d.ellipse([S(cx-r),S(cy-r),S(cx+r*0.6),S(cy+r*0.6)],fill=DARK+(200,))
    finish(im,"environment","rock.png",w,h,colors=14)

def rock_debris():
    w,h=64,32; im,d=canvas(w,h); rng=random.Random("deb")
    for cx in (12,30,48):
        _rock_blob(d,cx+rng.uniform(-3,3),16+rng.uniform(-4,4),rng.uniform(5,9),rng)
    finish(im,"environment","rock_debris.png",w,h,colors=12)

def wreck():
    w,h=128,96; im,d=canvas(w,h); rng=random.Random("wreck")
    # broken hull chunks (dark metal)
    for (x,y,ww,hh,rot) in [(30,40,60,26,0),(70,30,44,22,0),(24,60,40,18,0)]:
        d.polygon([(S(x),S(y)),(S(x+ww),S(y-4)),(S(x+ww-6),S(y+hh)),(S(x-4),S(y+hh-3))],fill=MET_D+(255,),outline=EDGE+(255,))
        d.line([(S(x),S(y+3)),(S(x+ww-6),S(y+2))],fill=A(MET_L,120),width=SS)
    # exposed frame ribs
    for i in range(4):
        rx=36+i*14; d.line([(S(rx),S(44)),(S(rx),S(66))],fill=MET+(255,),width=SS)
    # rust streaks + a dead ember
    for _ in range(8):
        sx=rng.uniform(28,96); sy=rng.uniform(34,72)
        d.rectangle([S(sx),S(sy),S(sx)+SS,S(sy)+SS*2],fill=A(RUST,120))
    glow(im,58,52,5,REDD,ORG,alpha=90)
    finish(im,"environment","wreck.png",w,h,colors=18)

def ruins():
    w,h=128,96; im,d=canvas(w,h); rng=random.Random("ruins")
    # broken stone columns/blocks
    for (x,ht) in [(24,50),(44,34),(60,58),(84,30),(100,46)]:
        d.rectangle([S(x),S(88-ht),S(x+12),S(90)],fill=ROCK+(255,),outline=EDGE+(255,))
        d.line([(S(x+1),S(88-ht)),(S(x+1),S(88))],fill=A(ROCK_LL,90),width=SS)
        # broken top
        d.polygon([(S(x),S(88-ht)),(S(x+12),S(88-ht)),(S(x+6),S(88-ht-5))],fill=ROCK_D+(255,))
    # base slab
    d.rectangle([S(16),S(88),S(112),S(94)],fill=ROCK_D+(255,),outline=EDGE+(255,))
    # faint runes
    for _ in range(4):
        rx=rng.uniform(28,100); ry=rng.uniform(50,84)
        d.rectangle([S(rx),S(ry),S(rx)+SS,S(ry)+SS*2],fill=A(CY,90))
    finish(im,"environment","ruins.png",w,h,colors=16)

def settlement():
    w,h=128,96; im,d=canvas(w,h); rng=random.Random("settle")
    # dark modular habs with lit windows
    for (x,y,ww,hh) in [(26,54,34,30),(64,46,40,38),(96,60,22,24)]:
        d.rectangle([S(x),S(y),S(x+ww),S(y+hh)],fill=MET_D+(255,),outline=EDGE+(255,))
        d.line([(S(x),S(y+1)),(S(x+ww),S(y+1))],fill=A(MET_L,110),width=SS)
        for wy in range(int(y+5),int(y+hh-3),7):
            for wx in range(int(x+4),int(x+ww-3),8):
                c=rng.choice([YEL,CY,YEL,(60,64,74)])
                d.rectangle([S(wx),S(wy),S(wx+3),S(wy+3)],fill=A(c,220))
    # antenna
    d.line([(S(80),S(46)),(S(80),S(32))],fill=MET_L+(255,),width=SS)
    d.ellipse([S(78),S(30),S(82),S(34)],fill=A(RED,220))
    finish(im,"environment","settlement.png",w,h,colors=18)

def thermal_vent():
    w=h=96; im,d=canvas(w,h); cx,cy=w/2,h/2+10; rng=random.Random("vent")
    _rock_blob(d,cx,cy,34,rng,base=ROCK_D)
    # glowing crack/vent
    glow(im,cx,cy-6,16,ORGD,YEL,alpha=200); d=ImageDraw.Draw(im)
    d.polygon([(S(cx-4),S(cy-22)),(S(cx+4),S(cy-22)),(S(cx+8),S(cy)),(S(cx-8),S(cy))],fill=A(ORG,220))
    d.polygon([(S(cx-2),S(cy-22)),(S(cx+2),S(cy-22)),(S(cx+3),S(cy)),(S(cx-3),S(cy))],fill=A(YEL,255))
    # heat wisps rising
    for _ in range(6):
        wx=cx+rng.uniform(-8,8); wy=cy-24-rng.uniform(0,20)
        d.rectangle([S(wx),S(wy),S(wx)+SS,S(wy)+SS*2],fill=A(ORG,90))
    finish(im,"environment","thermal_vent.png",w,h,colors=16)

def crystal_formation():
    w,h=64,48; im,d=canvas(w,h); rng=random.Random("cryst")
    _rock_blob(d,32,40,16,rng,base=ROCK_D)
    for (x,ht,col) in [(20,26,CY),(30,34,PURP),(40,22,CY),(46,30,PURP),(26,18,BIO)]:
        top=44-ht
        d.polygon([(S(x-3),S(44)),(S(x+3),S(44)),(S(x+1),S(top)),(S(x-1),S(top))],fill=mix(col,DARK,0.4)+(255,),outline=EDGE+(255,))
        d.line([(S(x),S(44)),(S(x),S(top))],fill=A(col,220),width=SS)
        d.rectangle([S(x-1),S(top),S(x+1),S(top+3)],fill=mix(col,WHITE,0.5)+(255,))
    glow(im,32,32,10,mix(CYD,PURPD,0.5),CY,alpha=90)
    finish(im,"environment","crystal_formation.png",w,h,colors=16)

def spore_growth():
    w,h=32,96; im,d=canvas(w,h); rng=random.Random("spore")
    # dark organic stalk rising
    x=w/2
    for i in range(6):
        y=90-i*14; r=8-i*0.8; x+=rng.uniform(-3,3)
        d.ellipse([S(x-r),S(y-r),S(x+r),S(y+r)],fill=mix((40,50,44),DARK,i/8)+(255,),outline=EDGE+(200,))
    # bioluminescent pods
    for _ in range(7):
        px=rng.uniform(8,24); py=rng.uniform(14,86)
        glow(im,px,py,3,GRND,BIOHI,alpha=150)
    finish(im,"environment","spore_growth.png",w,h,colors=14)

def bioluminescent_spot():
    w=h=32; im,d=canvas(w,h)
    glow(im,16,16,12,GRND,BIOHI,alpha=200)
    finish(im,"environment","bioluminescent_spot.png",w,h,colors=10)

def cave():
    w,h=128,96; im,d=canvas(w,h); rng=random.Random("cave")
    # rock mass with a dark maw
    pts=[]
    for a in range(0,360,20):
        rr=(52 if a<180 else 44)*(0.8+rng.random()*0.35)
        pts.append((S(64+math.cos(math.radians(a))*rr),S(52+math.sin(math.radians(a))*rr*0.8)))
    d.polygon(pts,fill=ROCK_D+(255,),outline=EDGE+(255,))
    # opening
    d.ellipse([S(44),S(38),S(84),S(74)],fill=DARK+(255,))
    d.ellipse([S(44),S(38),S(84),S(74)],outline=A(ROCK_L,120),width=SS)
    # faint inner glow + stalactite bits
    glow(im,64,58,8,(20,30,40),CYD,alpha=70); d=ImageDraw.Draw(im)
    for _ in range(6):
        sx=rng.uniform(48,80); d.polygon([(S(sx),S(40)),(S(sx+3),S(40)),(S(sx+1.5),S(46))],fill=ROCK+(255,))
    finish(im,"environment","cave.png",w,h,colors=16)

if __name__=="__main__":
    explosion(); enemy_projectile(); torpedo_trail(); shield_bubble(); sonar_ring(); electric_shock(); bubble()
    rock(); rock_debris(); wreck(); ruins(); settlement(); thermal_vent(); crystal_formation(); spore_growth(); bioluminescent_spot(); cave()
    print("done")
