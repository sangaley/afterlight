#!/usr/bin/env python3
"""Afterlight — combat effects + world objects in the code-generated style.
Each output at its exact native size (supersampled internally). Effects kept
light/neutral so the engine's per-type tint still works. Celestial art untouched."""
from PIL import Image, ImageDraw, ImageFilter
import os, math, random
SS=6
FX=os.path.join(os.path.dirname(__file__),"out2/effects"); EN=os.path.join(os.path.dirname(__file__),"out2/environment")
os.makedirs(FX,exist_ok=True); os.makedirs(EN,exist_ok=True)

ROCK_D=(52,54,60); ROCK=(88,90,98); ROCK_L=(120,124,132); EDGE=(24,28,34)
STEEL_D=(56,66,78); STEEL=(92,104,118); STEEL_L=(128,142,158); BOLT=(46,54,64)
ENERGY=(120,200,255); ENERGY_HI=(210,240,255); THRUST=(240,130,55); THR_HI=(255,215,130)
GREEN=(120,220,150); CYAN=(120,225,225); VIOLET=(170,130,235); VIOLET_HI=(220,200,255)
WHITE=(255,250,235); AMBER=(220,170,90)

def cv(w,h): return Image.new("RGBA",(w*SS,h*SS),(0,0,0,0))
def A(c,a): return (c[0],c[1],c[2],a)
def blur(img,r): return img.filter(ImageFilter.GaussianBlur(r*SS))
def finish(img,w,h,path): img.resize((w,h),Image.LANCZOS).save(path); print("·",os.path.basename(path),f"{w}x{h}")
def poly_rock(d,cx,cy,r,n,seed,base,edge=True):
    rnd=random.Random(seed); pts=[]
    for i in range(n):
        a=2*math.pi*i/n; rr=r*(0.72+0.28*rnd.random())
        pts.append((cx+math.cos(a)*rr,cy+math.sin(a)*rr))
    d.polygon(pts,fill=base+(255,),outline=(EDGE+(255,)) if edge else None)
    return pts

# ================= EFFECTS (tint-friendly / bright) =================
def torpedo_trail(w=32,h=8):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    for x in range(W):
        t=x/W; a=int(230*t*t)  # bright at head (right), fades to tail
        d.line([(x,H*0.5-t*H*0.35),(x,H*0.5+t*H*0.35)],fill=A(WHITE,a))
    d.ellipse([W-H,0,W,H],fill=A(WHITE,255))  # bright head
    finish(blur(img,0.6),w,h,f"{FX}/torpedo_trail.png")
def enemy_projectile(w=16,h=8):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    d.ellipse([0,1*SS,W,H-1*SS],fill=A((230,235,245),255))  # neutral bolt (engine tints)
    d.ellipse([W*0.45,2*SS,W-1*SS,H-2*SS],fill=A(WHITE,255))
    finish(blur(img,0.5),w,h,f"{FX}/enemy_projectile.png")
def bubble(w=16,h=16):
    img=cv(w,h); d=ImageDraw.Draw(img); W=w*SS
    d.ellipse([2*SS,2*SS,W-2*SS,W-2*SS],fill=A((180,210,235),70),outline=A((210,235,255),160),width=SS)
    d.ellipse([W*0.3,W*0.24,W*0.5,W*0.44],fill=A(WHITE,200))  # highlight
    finish(img,w,h,f"{FX}/bubble.png")
def electric_shock(w=32,h=32):
    img=cv(w,h); W,H=w*SS,h*SS; g=cv(w,h); gd=ImageDraw.Draw(g)
    rnd=random.Random(7); x,y=W*0.5,2*SS; pts=[(x,y)]
    while y<H-2*SS:
        y+=H/7; x=W*0.5+ (rnd.random()-0.5)*W*0.7; pts.append((x,y))
    gd.line(pts,fill=A(ENERGY,255),width=3*SS,joint="curve")
    img.alpha_composite(blur(g,2)); ImageDraw.Draw(img).line(pts,fill=A(ENERGY_HI,255),width=SS,joint="curve")
    finish(img,w,h,f"{FX}/electric_shock.png")
def explosion(w=48,h=48):
    img=cv(w,h); W=w*SS; c=W/2; g=cv(w,h); gd=ImageDraw.Draw(g)
    rnd=random.Random(3)
    for i in range(14):
        a=2*math.pi*i/14 + rnd.random()*0.3; ln=W*0.5*(0.7+0.3*rnd.random())
        gd.line([(c,c),(c+math.cos(a)*ln,c+math.sin(a)*ln)],fill=A(THRUST,220),width=4*SS)
    img.alpha_composite(blur(g,3)); d=ImageDraw.Draw(img)
    for r,col,a in [(W*0.42,THRUST,200),(W*0.30,THR_HI,235),(W*0.16,WHITE,255)]:
        gl=cv(w,h); ImageDraw.Draw(gl).ellipse([c-r,c-r,c+r,c+r],fill=col+(a,)); img.alpha_composite(blur(gl,2))
    for i in range(8):
        a=2*math.pi*i/8+0.4; dr=W*0.44; d.ellipse([c+math.cos(a)*dr-2*SS,c+math.sin(a)*dr-2*SS,c+math.cos(a)*dr+2*SS,c+math.sin(a)*dr+2*SS],fill=A((60,50,46),220))
    finish(img,w,h,f"{FX}/explosion.png")
def sonar_ring(w=64,h=64):
    img=cv(w,h); d=ImageDraw.Draw(img); W=w*SS; c=W/2
    for r,a in [(W*0.46,255),(W*0.34,120)]:
        d.ellipse([c-r,c-r,c+r,c+r],outline=A(GREEN,a),width=2*SS)
    finish(blur(img,0.6),w,h,f"{FX}/sonar_ring.png")
def shield_bubble(w=256,h=256):
    img=cv(w,h); d=ImageDraw.Draw(img); W=w*SS; c=W/2; R=W*0.47
    ring=cv(w,h); ImageDraw.Draw(ring).ellipse([c-R,c-R,c+R,c+R],outline=A(ENERGY,230),width=5*SS)
    img.alpha_composite(blur(ring,3))
    d.ellipse([c-R,c-R,c+R,c+R],outline=A(ENERGY_HI,180),width=2*SS)
    fill=cv(w,h); ImageDraw.Draw(fill).ellipse([c-R,c-R,c+R,c+R],fill=A(ENERGY,26)); img.alpha_composite(fill)
    # faint hex facets
    for i in range(6):
        a=2*math.pi*i/6; hx=c+math.cos(a)*R*0.6; hy=c+math.sin(a)*R*0.6
        d.regular_polygon((hx,hy,R*0.18),6,rotation=0,outline=A(ENERGY,40),fill=None)
    finish(img,w,h,f"{FX}/shield_bubble.png")

# ================= WORLD OBJECTS =================
def wreck(w=128,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    # broken dark hull chunks (my module look, darkened + jagged)
    for (x0,y0,x1,y1) in [(W*0.12,H*0.32,W*0.52,H*0.66),(W*0.42,H*0.2,W*0.7,H*0.5),(W*0.56,H*0.5,W*0.86,H*0.78)]:
        d.rounded_rectangle([x0,y0,x1,y1],radius=6*SS,fill=STEEL_D+(255,),outline=EDGE+(255,),width=2*SS)
        for gx in range(int(x0)+10*SS,int(x1)-6*SS,16*SS): d.line([(gx,y0+4*SS),(gx,y1-4*SS)],fill=EDGE+(150,),width=SS)
    # jagged break edge
    d.line([(W*0.52,H*0.32),(W*0.58,H*0.44),(W*0.5,H*0.54),(W*0.6,H*0.66)],fill=EDGE+(220,),width=3*SS)
    # a dead reactor (dim)
    d.ellipse([W*0.24,H*0.42,W*0.4,H*0.58],fill=(40,54,66)+(255,),outline=EDGE+(255,),width=2*SS)
    d.ellipse([W*0.29,H*0.47,W*0.35,H*0.53],fill=A((70,110,140),200))
    # scattered debris
    for s in range(6): poly_rock(d,W*(0.15+0.7*random.Random(s).random()),H*(0.1+0.85*random.Random(s+9).random()),5*SS,6,s,ROCK_D)
    finish(img,w,h,f"{EN}/wreck.png")
def cave(w=128,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    poly_rock(d,W/2,H/2,W*0.46,14,5,ROCK)  # big rock mass
    # top-light
    tl=cv(w,h); ImageDraw.Draw(tl).ellipse([W*0.1,-H*0.3,W*0.9,H*0.7],fill=A(ROCK_L,60)); img.alpha_composite(blur(tl,4))
    d=ImageDraw.Draw(img)
    d.ellipse([W*0.34,H*0.32,W*0.66,H*0.74],fill=(10,12,16,255))  # dark mouth
    d.ellipse([W*0.38,H*0.36,W*0.62,H*0.7],fill=(6,8,11,255))
    for s in range(5): poly_rock(d,W*(0.2+0.6*random.Random(s+3).random()),H*(0.2+0.6*random.Random(s+7).random()),4*SS,6,s+1,ROCK_D)
    finish(img,w,h,f"{EN}/cave.png")
def ruins(w=128,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    # angular alien monoliths with faint violet glow
    for (x,bw,bh) in [(W*0.22,14*SS,H*0.55),(W*0.44,12*SS,H*0.7),(W*0.62,16*SS,H*0.45),(W*0.8,10*SS,H*0.6)]:
        top=H-bh
        d.polygon([(x-bw,H*0.92),(x+bw,H*0.92),(x+bw-3*SS,top),(x-bw+3*SS,top)],fill=(64,60,80)+(255,),outline=EDGE+(255,))
        d.line([(x,top+4*SS),(x,H*0.9)],fill=A(VIOLET,120),width=2*SS)  # glowing seam
    # broken lintel
    d.polygon([(W*0.18,H*0.4),(W*0.52,H*0.28),(W*0.5,H*0.36),(W*0.2,H*0.48)],fill=(54,50,68)+(255,),outline=EDGE+(255,))
    g=cv(w,h); ImageDraw.Draw(g).ellipse([W*0.3,H*0.5,W*0.7,H*0.95],fill=A(VIOLET,50)); img.alpha_composite(blur(g,5))
    finish(img,w,h,f"{EN}/ruins.png")
def thermal_vent(w=96,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    poly_rock(d,W/2,H*0.8,W*0.4,10,4,ROCK_D)  # vent base
    d.polygon([(W*0.4,H*0.86),(W*0.6,H*0.86),(W*0.54,H*0.52),(W*0.46,H*0.52)],fill=(40,44,50)+(255,),outline=EDGE+(255,))  # chimney
    # heat plume
    for r,col,a in [(0,None,0)]:
        pass
    g=cv(w,h); gd=ImageDraw.Draw(g)
    gd.polygon([(W*0.44,H*0.54),(W*0.56,H*0.54),(W*0.64,H*0.1),(W*0.36,H*0.1)],fill=A(THRUST,150)); img.alpha_composite(blur(g,4))
    g=cv(w,h); ImageDraw.Draw(g).polygon([(W*0.47,H*0.54),(W*0.53,H*0.54),(W*0.57,H*0.18),(W*0.43,H*0.18)],fill=A(THR_HI,180)); img.alpha_composite(blur(g,3))
    d=ImageDraw.Draw(img); d.ellipse([W*0.46,H*0.5,W*0.54,H*0.58],fill=A(THR_HI,230))
    finish(img,w,h,f"{EN}/thermal_vent.png")
def settlement(w=128,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    # central hub + arms (small station)
    d.ellipse([W*0.4,H*0.34,W*0.6,H*0.62],fill=STEEL+(255,),outline=EDGE+(255,),width=2*SS)
    d.ellipse([W*0.44,H*0.38,W*0.56,H*0.5],fill=A(ENERGY,120))
    for (x0,y0,x1,y1) in [(W*0.12,H*0.42,W*0.4,H*0.54),(W*0.6,H*0.42,W*0.88,H*0.54),(W*0.44,H*0.1,W*0.56,H*0.34),(W*0.44,H*0.62,W*0.56,H*0.86)]:
        d.rounded_rectangle([x0,y0,x1,y1],radius=4*SS,fill=STEEL_D+(255,),outline=EDGE+(255,),width=2*SS)
    # module pods at arm ends
    for (x,y) in [(W*0.14,H*0.48),(W*0.86,H*0.48),(W*0.5,H*0.12),(W*0.5,H*0.84)]:
        d.rounded_rectangle([x-9*SS,y-9*SS,x+9*SS,y+9*SS],radius=3*SS,fill=STEEL+(255,),outline=EDGE+(255,),width=2*SS)
    for (x,y) in [(W*0.14,H*0.44),(W*0.86,H*0.44),(W*0.5,H*0.86)]:
        d.ellipse([x-2*SS,y-2*SS,x+2*SS,y+2*SS],fill=A((255,210,120),255))  # window lights
    finish(img,w,h,f"{EN}/settlement.png")
def rock(w=48,h=48):
    img=cv(w,h); d=ImageDraw.Draw(img); W=w*SS
    poly_rock(d,W/2,W/2,W*0.42,9,11,ROCK)
    tl=cv(w,h); ImageDraw.Draw(tl).ellipse([W*0.1,W*0.02,W*0.9,W*0.6],fill=A(ROCK_L,70)); img.alpha_composite(blur(tl,3))
    d=ImageDraw.Draw(img)
    for s in range(3): x=W*(0.3+0.4*random.Random(s).random()); y=W*(0.4+0.4*random.Random(s+2).random()); d.ellipse([x-3*SS,y-3*SS,x+3*SS,y+3*SS],fill=A(ROCK_D,200))
    finish(img,w,h,f"{EN}/rock.png")
def spore_growth(w=32,h=96):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    rnd=random.Random(6)
    for i in range(3):
        bx=W*(0.3+0.2*i); base=H*0.95; top=H*(0.5-0.15*rnd.random())
        d.line([(bx,base),(bx+ (rnd.random()-0.5)*W*0.2,top)],fill=(70,110,80)+(255,),width=5*SS)
        tipx=bx+(rnd.random()-0.5)*W*0.2
        gl=cv(w,h); ImageDraw.Draw(gl).ellipse([tipx-7*SS,top-7*SS,tipx+7*SS,top+7*SS],fill=A(GREEN,200)); img.alpha_composite(blur(gl,2))
        d.ellipse([tipx-4*SS,top-4*SS,tipx+4*SS,top+4*SS],fill=A((200,255,210),230))
    finish(img,w,h,f"{EN}/spore_growth.png")
def crystal_formation(w=64,h=48):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    rnd=random.Random(2)
    for i in range(4):
        bx=W*(0.2+0.2*i); bw=6*SS+rnd.random()*4*SS; top=H*(0.15+0.3*rnd.random())
        d.polygon([(bx-bw,H*0.92),(bx+bw,H*0.92),(bx+bw*0.3,top),(bx-bw*0.3,top)],fill=A(VIOLET,235),outline=EDGE+(255,))
        d.line([(bx-bw*0.2,H*0.9),(bx,top+3*SS)],fill=A(VIOLET_HI,180),width=SS)
    g=cv(w,h); ImageDraw.Draw(g).ellipse([W*0.2,H*0.5,W*0.8,H],fill=A(VIOLET,50)); img.alpha_composite(blur(g,4))
    finish(img,w,h,f"{EN}/crystal_formation.png")
def bioluminescent_spot(w=32,h=32):
    img=cv(w,h); W=w*SS; c=W/2
    for r,a in [(W*0.45,60),(W*0.3,120),(W*0.16,230)]:
        g=cv(w,h); ImageDraw.Draw(g).ellipse([c-r,c-r,c+r,c+r],fill=A(CYAN,a)); img.alpha_composite(blur(g,2))
    ImageDraw.Draw(img).ellipse([c-3*SS,c-3*SS,c+3*SS,c+3*SS],fill=A((220,255,255),255))
    finish(img,w,h,f"{EN}/bioluminescent_spot.png")
def rock_debris(w=64,h=32):
    img=cv(w,h); d=ImageDraw.Draw(img); W,H=w*SS,h*SS
    for s in range(5):
        x=W*(0.12+0.76*random.Random(s).random()); y=H*(0.2+0.6*random.Random(s+4).random())
        poly_rock(d,x,y,(3+random.Random(s+1).random()*4)*SS,6,s+20,ROCK)
    finish(img,w,h,f"{EN}/rock_debris.png")

for fn in [torpedo_trail,enemy_projectile,bubble,electric_shock,explosion,sonar_ring,shield_bubble,
           wreck,cave,ruins,thermal_vent,settlement,rock,spore_growth,crystal_formation,bioluminescent_spot,rock_debris]:
    fn()
print("ALL DONE")
