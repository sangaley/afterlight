#!/usr/bin/env python3
"""Generate every module as a furnished top-down room, at its true footprint.
Shares primitives from rooms_lib. Directional modules (engines/weapons) keep
their machinery pointing 'up' (North) so the engine rotation offset holds."""
from rooms_lib import *
import math

# ============================================================ POWER
def small_reactor(r):
    r.deck(tint=ENERGY)
    S=r; cx,cy=S.fx(0.5),S.fy(0.5)
    # coolant fins around housing
    for a in range(8):
        ang=a*math.pi/4; rr=S.u(30)
        fx,fy=cx+math.cos(ang)*rr,cy+math.sin(ang)*rr
        r.d.line([(cx,cy),(fx,fy)],fill=STEEL_D+(255,),width=S.u(3))
    r.glow_core(cx,cy,S.u(24),ENERGY,ENERGY_HI)
    # control screen bottom
    r.screen([S.fx(0.34),S.fy(0.80),S.fx(0.66),S.fy(0.90)],col=ENERGY)
    r.walls(accent=ENERGY)

def large_reactor(r):
    r.deck(tint=ENERGY)
    S=r; x0,y0,x1,y1=r.box()
    r.hazard_edge(True,True)
    # coolant pipes along length
    for fy in (0.30,0.70):
        r.pipe(S.fx(0.10),S.fy(fy),S.fx(0.90),S.fy(fy),w=6,col=STEEL_L)
    cx,cy=S.fx(0.5),S.fy(0.5)
    R=min(S.H,S.W)*0.30
    r.glow_core(cx,cy,R,ENERGY,ENERGY_HI)
    if r.cw>=3:  # fusion 3x3: satellite coils + more consoles
        for (fx,fy) in [(0.22,0.24),(0.78,0.24),(0.22,0.76),(0.78,0.76)]:
            r.glow_core(S.fx(fx),S.fy(fy),S.u(12),ENERGY_D,ENERGY,rings=False)
    # end consoles
    r.panel([x0+S.u(6),S.fy(0.36),x0+S.u(6)+S.u(20),S.fy(0.64)])
    r.screen([x0+S.u(9),S.fy(0.40),x0+S.u(6)+S.u(17),S.fy(0.60)],col=ENERGY)
    r.panel([x1-S.u(6)-S.u(20),S.fy(0.36),x1-S.u(6),S.fy(0.64)])
    r.screen([x1-S.u(6)-S.u(17),S.fy(0.40),x1-S.u(9),S.fy(0.60)],col=AMBER)
    r.walls(accent=ENERGY)

def battery(r):
    r.deck(tint=ENERGY)
    S=r; x0,y0,x1,y1=r.box()
    # bus bar across top
    r.pipe(S.fx(0.14),S.fy(0.20),S.fx(0.86),S.fy(0.20),w=5,col=AMBER)
    # rows of cylindrical cells with charge LEDs
    for row in range(2):
        for col in range(3):
            bx=S.fx(0.18+col*0.24); by=S.fy(0.40+row*0.28)
            bw=S.u(14); bh=S.u(18)
            r.panel([bx-bw/2,by-bh/2,bx+bw/2,by+bh/2],fill=STEEL_D)
            r.d.rounded_rectangle([bx-bw/2+S.u(2),by-bh/2+S.u(2),bx+bw/2-S.u(2),by+bh/2-S.u(2)],
                                  radius=S.u(2),outline=A(ENERGY,120),width=SS)
            # charge level
            lv=[0.9,0.7,1.0,0.6,0.85,0.75][row*3+col]
            r.d.rectangle([bx-bw/2+S.u(3),by+bh/2-S.u(3)-(bh-S.u(6))*lv,bx+bw/2-S.u(3),by+bh/2-S.u(3)],
                          fill=A(ENERGY,180))
            r.d.ellipse([bx-S.u(1.5),by-bh/2-S.u(2),bx+S.u(1.5),by-bh/2+S.u(1)],fill=LED_G+(255,))
    r.walls(accent=ENERGY)

# ============================================================ PROPULSION (directional, exhaust DOWN)
def _engine_body(r, twin=False):
    S=r; x0,y0,x1,y1=r.box()
    cols=[0.5] if not twin else [0.30,0.70]
    for cxf in cols:
        cx=S.fx(cxf)
        # intake/machinery block (top)
        bw=S.u(26) if not twin else S.u(20)
        r.panel([cx-bw/2,S.fy(0.14),cx+bw/2,S.fy(0.56)],fill=STEEL)
        for gy in (0.22,0.30,0.38,0.46):
            r.d.line([(cx-bw/2+S.u(3),S.fy(gy)),(cx+bw/2-S.u(3),S.fy(gy))],fill=A(EDGE,150),width=SS)
        # combustion chamber (trapezoid narrowing down)
        r.d.polygon([(cx-bw*0.42,S.fy(0.56)),(cx+bw*0.42,S.fy(0.56)),
                     (cx+bw*0.30,S.fy(0.72)),(cx-bw*0.30,S.fy(0.72))],fill=STEEL_D+(255,),outline=EDGE+(255,))
        # nozzle + exhaust glow DOWN
        g=Image.new("RGBA",r.img.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
        for i in range(20,0,-1):
            t=i/20; w=bw*0.30*(1+0.8*(1-t)); yy=S.fy(0.72)+(S.fy(0.96)-S.fy(0.72))*(1-t)
            c=mix(THRUST,THRUST_HI,t)
            gd.polygon([(cx-w,yy),(cx+w,yy),(cx+w*0.7,yy+S.u(3)),(cx-w*0.7,yy+S.u(3))],fill=c+(int(50+150*t),))
        r.img.alpha_composite(blur(g,1.6)); r.d=ImageDraw.Draw(r.img)

def standard_engine(r):
    r.deck(tint=THRUST)
    _engine_body(r, twin=(r.cw>=2))
    r.walls(accent=THRUST)

def silent_drive(r):
    r.deck(tint=(70,90,120))
    S=r; cx=S.fx(0.5)
    # shrouded, baffled — muted
    r.panel([cx-S.u(24),S.fy(0.16),cx+S.u(24),S.fy(0.58)],fill=DARK)
    for gy in (0.22,0.30,0.38,0.46,0.54):  # sound baffles
        r.d.line([(cx-S.u(21),S.fy(gy)),(cx+S.u(21),S.fy(gy))],fill=A((90,110,140),160),width=S.u(2))
    r.d.polygon([(cx-S.u(18),S.fy(0.58)),(cx+S.u(18),S.fy(0.58)),(cx+S.u(12),S.fy(0.74)),(cx-S.u(12),S.fy(0.74))],
                fill=STEEL_D+(255,),outline=EDGE+(255,))
    # muted blue exhaust
    g=Image.new("RGBA",r.img.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
    for i in range(16,0,-1):
        t=i/16; w=S.u(11)*(1+0.7*(1-t)); yy=S.fy(0.74)+(S.fy(0.92)-S.fy(0.74))*(1-t)
        gd.polygon([(cx-w,yy),(cx+w,yy)],fill=mix((70,120,170),(150,190,230),t)+(int(30+90*t),))
    r.img.alpha_composite(blur(g,2.4)); r.d=ImageDraw.Draw(r.img)
    r.walls(accent=(90,120,160))

# ============================================================ LIFE SUPPORT
def oxygen_scrubber(r):
    r.deck(tint=GREEN)
    S=r; x0,y0,x1,y1=r.box()
    n=2 if r.cw<2 else 4
    for i in range(n):
        cx=S.fx((i+0.5)/n)
        # O2 cylinder tank
        tw=S.u(16); th=S.u(40)
        r.panel([cx-tw/2,S.fy(0.22),cx+tw/2,S.fy(0.22)+th],fill=STEEL,rad=8)
        r.d.rounded_rectangle([cx-tw/2+S.u(2),S.fy(0.26),cx+tw/2-S.u(2),S.fy(0.22)+th-S.u(3)],
                              radius=S.u(6),outline=A(GREEN,120),width=SS)
        # green label band + O2 mark
        r.d.rectangle([cx-tw/2+S.u(1),S.fy(0.40),cx+tw/2-S.u(1),S.fy(0.48)],fill=A(GREEN,150))
        r.d.ellipse([cx-S.u(3),S.fy(0.42),cx+S.u(3),S.fy(0.47)],outline=GREEN_HI+(220,),width=SS)
        # gauge on top
        r.d.ellipse([cx-S.u(4),S.fy(0.18),cx+S.u(4),S.fy(0.26)],fill=SCREEN+(255,),outline=STEEL_D+(255,),width=SS)
        r.d.line([(cx,S.fy(0.22)),(cx+S.u(2),S.fy(0.20))],fill=GREEN_HI+(255,),width=SS)
    # scrubber fan bottom
    r.panel([S.fx(0.30),S.fy(0.78),S.fx(0.70),S.fy(0.90)],fill=STEEL_D)
    r.walls(accent=GREEN)

def life_support(r):
    r.deck(tint=GREEN)
    S=r
    # filter canisters row
    for i in range(3):
        cx=S.fx(0.22+i*0.28)
        r.panel([cx-S.u(11),S.fy(0.20),cx+S.u(11),S.fy(0.54)],fill=STEEL)
        for gy in (0.28,0.36,0.44):
            r.d.line([(cx-S.u(8),S.fy(gy)),(cx+S.u(8),S.fy(gy))],fill=A(GREEN_D,150),width=SS)
    # air ducts
    r.pipe(S.fx(0.14),S.fy(0.64),S.fx(0.86),S.fy(0.64),w=6,col=STEEL_L)
    r.pipe(S.fx(0.14),S.fy(0.64),S.fx(0.14),S.fy(0.80),w=6,col=STEEL_L)
    # atmosphere gauge
    r.screen([S.fx(0.40),S.fy(0.74),S.fx(0.72),S.fy(0.88)],col=GREEN)
    r.walls(accent=GREEN)

# ============================================================ WEAPONS (directional, muzzle UP)
def _turret(r, barrels=2, long=False):
    S=r; cx=S.fx(0.5); cy=S.fy(0.62 if not long else 0.66)
    # ammo feed box behind
    r.panel([cx-S.u(18),cy-S.u(2),cx+S.u(18),cy+S.u(20)],fill=STEEL_D)
    # rotating base ring
    r.d.ellipse([cx-S.u(20),cy-S.u(20),cx+S.u(20),cy+S.u(20)],fill=STEEL+(255,),outline=EDGE+(255,),width=S.u(2))
    r.d.ellipse([cx-S.u(14),cy-S.u(14),cx+S.u(14),cy+S.u(14)],fill=STEEL_D+(255,),outline=A(STEEL_L,80),width=SS)
    r.bolts([cx-S.u(18),cy-S.u(18),cx+S.u(18),cy+S.u(18)])
    # barrels pointing UP (native North)
    top=S.fy(0.10 if not long else 0.06)
    offs=[0] if barrels==1 else [-S.u(6),S.u(6)]
    for ox in offs:
        bw=S.u(5)
        r.d.rounded_rectangle([cx+ox-bw/2,top,cx+ox+bw/2,cy],radius=S.u(1),fill=STEEL_L+(255,),outline=EDGE+(255,),width=SS)
        r.d.rectangle([cx+ox-bw/2-S.u(1),top,cx+ox+bw/2+S.u(1),top+S.u(4)],fill=STEEL_D+(255,))  # muzzle brake
    return cx,cy

def point_defense(r):
    r.deck(tint=(90,96,110))
    _turret(r,barrels=2)
    r.walls(accent=AMBER)

def railgun(r):
    r.deck(tint=(90,96,110))
    S=r
    cx,cy=_turret(r,barrels=1,long=True)
    # segmented rail up the barrel
    top=S.fy(0.06)
    for i in range(5):
        yy=top+(cy-top)*i/5
        r.d.rectangle([cx-S.u(7),yy,cx+S.u(7),yy+S.u(2)],fill=A(ENERGY,150))
    r.walls(accent=ENERGY)

def torpedo_tube(r):
    r.deck(tint=(90,96,110))
    S=r; cx=S.fx(0.5)
    # launch tubes with missile tips (up)
    for ox in (-S.u(9),S.u(9)):
        r.panel([cx+ox-S.u(6),S.fy(0.16),cx+ox+S.u(6),S.fy(0.74)],fill=STEEL_D,rad=3)
        r.d.polygon([(cx+ox-S.u(4),S.fy(0.22)),(cx+ox+S.u(4),S.fy(0.22)),(cx+ox,S.fy(0.14))],fill=RED+(255,))  # warhead tip
        r.d.rectangle([cx+ox-S.u(4),S.fy(0.24),cx+ox+S.u(4),S.fy(0.60)],fill=A(STEEL_L,120))
    r.walls(accent=RED)

def mine_layer(r):
    r.deck(tint=(90,96,110))
    S=r
    # rack of rockets/mines
    for i in range(3):
        cx=S.fx(0.24+i*0.26)
        r.panel([cx-S.u(7),S.fy(0.20),cx+S.u(7),S.fy(0.70)],fill=STEEL_D,rad=3)
        r.d.polygon([(cx-S.u(5),S.fy(0.26)),(cx+S.u(5),S.fy(0.26)),(cx,S.fy(0.18))],fill=AMBER+(255,))
        r.d.ellipse([cx-S.u(3),S.fy(0.50),cx+S.u(3),S.fy(0.56)],fill=A(RED,180))
    r.walls(accent=AMBER)

def salvage_arm(r):
    r.deck(tint=(96,100,110))
    S=r; bx,by=S.fx(0.5),S.fy(0.74)
    # base
    r.d.ellipse([bx-S.u(16),by-S.u(10),bx+S.u(16),by+S.u(12)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=S.u(2))
    # articulated arm up
    j1=(bx,by); j2=(S.fx(0.38),S.fy(0.44)); j3=(S.fx(0.58),S.fy(0.22))
    for a,b in [(j1,j2),(j2,j3)]:
        r.d.line([a,b],fill=STEEL_L+(255,),width=S.u(6)); r.d.line([a,b],fill=A(EDGE,120),width=SS)
    for j in (j1,j2,j3):
        r.d.ellipse([j[0]-S.u(4),j[1]-S.u(4),j[0]+S.u(4),j[1]+S.u(4)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)
    # grapple claw
    cxx,cyy=j3
    r.d.line([(cxx,cyy),(cxx-S.u(7),cyy-S.u(9))],fill=AMBER+(255,),width=S.u(3))
    r.d.line([(cxx,cyy),(cxx+S.u(7),cyy-S.u(9))],fill=AMBER+(255,),width=S.u(3))
    r.walls(accent=AMBER)

# ============================================================ SENSORS
def sonar_array(r):
    r.deck(tint=CYAN)
    S=r; cx,cy=S.fx(0.5),S.fy(0.46)
    # dish mount
    r.panel([cx-S.u(8),S.fy(0.70),cx+S.u(8),S.fy(0.86)],fill=STEEL_D)
    # parabolic dish
    R=S.u(26)
    r.d.ellipse([cx-R,cy-R*0.9,cx+R,cy+R*0.9],fill=STEEL+(255,),outline=EDGE+(255,),width=S.u(2))
    r.d.ellipse([cx-R+S.u(3),cy-R*0.9+S.u(3),cx+R-S.u(3),cy+R*0.9-S.u(3)],fill=STEEL_D+(255,))
    for rr in (0.7,0.45,0.2):  # dish ribs
        r.d.ellipse([cx-R*rr,cy-R*0.9*rr,cx+R*rr,cy+R*0.9*rr],outline=A(CYAN,120),width=SS)
    # feed horn
    r.d.line([(cx,cy),(cx,cy-R*0.7)],fill=STEEL_L+(255,),width=S.u(2))
    r.d.ellipse([cx-S.u(3),cy-R*0.7-S.u(3),cx+S.u(3),cy-R*0.7+S.u(3)],fill=CYAN_HI if False else CYAN+(255,))
    r.walls(accent=CYAN)

def passive_sonar(r):
    r.deck(tint=CYAN)
    S=r; n=1 if r.cw<2 else 2
    for k in range(n):
        ox=S.fx((k+0.5)/n)-S.fx(0.5)
        # hydrophone strip array
        for i in range(4):
            yy=S.fy(0.20+i*0.13)
            r.d.rounded_rectangle([S.fx(0.5)+ox-S.u(18),yy,S.fx(0.5)+ox+S.u(18),yy+S.u(6)],radius=S.u(2),
                                  fill=STEEL_D+(255,),outline=EDGE+(200,),width=SS)
            for j in range(5):
                dx=S.fx(0.5)+ox-S.u(15)+j*S.u(7.5)
                r.d.ellipse([dx-S.u(1.5),yy+S.u(1),dx+S.u(1.5),yy+S.u(4)],fill=A(CYAN,180))
    # waveform readout bottom
    r.screen([S.fx(0.30),S.fy(0.78),S.fx(0.70),S.fy(0.90)],col=CYAN)
    r.walls(accent=CYAN)

def depth_sensor(r):
    r.deck(tint=CYAN)
    S=r; cx,cy=S.fx(0.5),S.fy(0.44)
    # scope screen with radial sweep
    R=S.u(24)
    r.d.ellipse([cx-R,cy-R,cx+R,cy+R],fill=SCREEN+(255,),outline=STEEL_D+(255,),width=S.u(3))
    for rr in (0.33,0.66,1.0):
        r.d.ellipse([cx-R*rr,cy-R*rr,cx+R*rr,cy+R*rr],outline=A(CYAN,110),width=SS)
    r.d.line([(cx,cy),(cx,cy-R)],fill=A(CYAN,90),width=SS)
    r.d.line([(cx,cy),(cx+R*0.8,cy-R*0.6)],fill=CYAN+(255,),width=S.u(2))  # sweep
    r.d.ellipse([cx+R*0.5-S.u(2),cy-R*0.3-S.u(2),cx+R*0.5+S.u(2),cy-R*0.3+S.u(2)],fill=LED_G+(255,))  # blip
    # emitter box
    r.panel([S.fx(0.34),S.fy(0.78),S.fx(0.66),S.fy(0.90)],fill=STEEL_D)
    r.walls(accent=CYAN)

# ============================================================ STORAGE
def cargo_hold(r):
    r.deck(tint=AMBER)
    S=r; x0,y0,x1,y1=r.box()
    # grid of shipping crates sized ~ to cells
    nx=max(2,r.cw*2); ny=max(2,r.ch*2)
    pad=S.u(3)
    cw=(x1-x0-S.u(8))/nx; ch=(y1-y0-S.u(8))/ny
    cols=[STEEL_D,mix(AMBER,STEEL,0.5),STEEL]
    for iy in range(ny):
        for ix in range(nx):
            if (ix*7+iy*3)%5==4: continue  # a gap for walkway feel
            bx0=x0+S.u(4)+ix*cw+pad; by0=y0+S.u(4)+iy*ch+pad
            bx1=x0+S.u(4)+(ix+1)*cw-pad; by1=y0+S.u(4)+(iy+1)*ch-pad
            col=cols[(ix+iy)%3]
            r.panel([bx0,by0,bx1,by1],fill=col,shadow=(ix+iy)%2==0)
            # strap cross
            r.d.line([((bx0+bx1)/2,by0+SS),((bx0+bx1)/2,by1-SS)],fill=A(AMBER_D,160),width=SS)
            r.d.line([(bx0+SS,(by0+by1)/2),(bx1-SS,(by0+by1)/2)],fill=A(AMBER_D,160),width=SS)
    r.walls(accent=AMBER)

def ballast_tank(r):
    r.deck(tint=(90,110,120))
    S=r; cx,cy=S.fx(0.5),S.fy(0.5)
    # big cylindrical tank
    tw=S.u(30); th=S.u(52)
    r.panel([cx-tw/2,cy-th/2,cx+tw/2,cy+th/2],fill=STEEL,rad=12)
    # fluid level
    r.d.rounded_rectangle([cx-tw/2+S.u(4),cy-th/2+S.u(20),cx+tw/2-S.u(4),cy+th/2-S.u(4)],radius=S.u(6),fill=A((90,150,180),150))
    # bands
    for fy in (0.30,0.50,0.70):
        r.d.line([(cx-tw/2,S.fy(fy)),(cx+tw/2,S.fy(fy))],fill=A(EDGE,120),width=SS)
    # valves top
    r.d.ellipse([cx-S.u(5),cy-th/2-S.u(3),cx+S.u(5),cy-th/2+S.u(6)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS)
    r.walls(accent=(110,160,190))

def research_lab(r):
    r.deck(tint=(120,150,170))
    S=r
    big = r.cw>=2
    # containment tube (with creature silhouette)
    cxx=S.fx(0.28 if big else 0.32)
    tw=S.u(20 if big else 16); th=S.u(46)
    r.panel([cxx-tw/2,S.fy(0.16),cxx+tw/2,S.fy(0.16)+th],fill=STEEL_D,rad=10)
    r.d.rounded_rectangle([cxx-tw/2+S.u(3),S.fy(0.20),cxx+tw/2-S.u(3),S.fy(0.16)+th-S.u(4)],radius=S.u(6),fill=A((90,200,190),110))
    # creature silhouette
    r.d.ellipse([cxx-S.u(6),S.fy(0.40),cxx+S.u(6),S.fy(0.56)],fill=A((30,50,60),200))
    for a in range(5):
        ang=math.pi*(0.2+a*0.15); ex=cxx+math.cos(ang)*S.u(9); ey=S.fy(0.50)+math.sin(ang)*S.u(9)
        r.d.line([(cxx,S.fy(0.50)),(ex,ey)],fill=A((30,50,60),180),width=S.u(2))
    # lab bench + beakers
    bx=S.fx(0.62)
    r.panel([bx-S.u(4),S.fy(0.30),S.fx(0.86),S.fy(0.42)],fill=STEEL)
    for i,cc in enumerate([(120,220,150),(220,180,90),(160,120,220)]):
        r.d.rectangle([bx+i*S.u(9),S.fy(0.24),bx+i*S.u(9)+S.u(5),S.fy(0.30)],fill=A(cc,200))
    r.screen([S.fx(0.58),S.fy(0.60),S.fx(0.88),S.fy(0.80)],col=(120,200,190),grid=True)
    r.walls(accent=(120,200,190))

# ============================================================ CREW
def _bunk(r, x, y, w, h, blanket):
    S=r
    r.panel([x,y,x+w,y+h],fill=STEEL_D,rad=3,shadow=True)
    r.d.rounded_rectangle([x+S.u(2),y+S.u(2),x+w-S.u(2),y+h-S.u(2)],radius=S.u(2),fill=LINEN+(255,))
    r.d.rounded_rectangle([x+S.u(3),y+S.u(3),x+w-S.u(3),y+h*0.30],radius=S.u(2),fill=PILLOW+(255,))  # pillow
    r.d.rounded_rectangle([x+S.u(2),y+h*0.42,x+w-S.u(2),y+h-S.u(2)],radius=S.u(2),fill=blanket+(255,))

# reuse medbay linen colors
LINEN=(228,234,240); PILLOW=(240,244,250); SKIN=(214,180,158)

def basic_quarters(r):
    S=r; x0,y0,x1,y1=r.box()
    if r.cw>=3:   # WellnessHub 3x3 — lounge
        r.deck(tint=(150,140,120))
        # central low table + surrounding couches
        cx,cy=S.fx(0.5),S.fy(0.5)
        r.panel([cx-S.u(22),cy-S.u(14),cx+S.u(22),cy+S.u(14)],fill=(120,96,70),rad=4)  # wood table
        for (px,py,w,h) in [(0.5,0.18,0.5,0.10),(0.5,0.82,0.5,0.10),(0.16,0.5,0.10,0.5),(0.84,0.5,0.10,0.5)]:
            r.panel([S.fx(px)-S.W*w*0.5*0.5,S.fy(py)-S.H*h*0.5*0.5,S.fx(px)+S.W*w*0.5*0.5,S.fy(py)+S.H*h*0.5*0.5],fill=(70,90,120),rad=6)
        # plants in corners
        for (fx,fy) in [(0.14,0.14),(0.86,0.14),(0.14,0.86),(0.86,0.86)]:
            r.d.ellipse([S.fx(fx)-S.u(6),S.fy(fy)-S.u(6),S.fx(fx)+S.u(6),S.fy(fy)+S.u(6)],fill=(60,140,80,255))
        r.walls(accent=(150,180,140)); return
    if r.cw==2 and r.ch==2:  # GalleyMess 2x2 — dining + kitchen
        r.deck(tint=(150,140,120))
        # kitchen counter along top
        r.panel([S.fx(0.10),S.fy(0.12),S.fx(0.90),S.fy(0.24)],fill=STEEL)
        r.d.ellipse([S.fx(0.20),S.fy(0.14),S.fx(0.28),S.fy(0.22)],fill=SCREEN+(255,))  # stove
        r.d.rectangle([S.fx(0.60),S.fy(0.14),S.fx(0.74),S.fy(0.22)],fill=A(CYAN,120))    # sink
        # two dining tables with benches
        for tyf in (0.50,0.78):
            r.panel([S.fx(0.24),S.fy(tyf)-S.u(6),S.fx(0.76),S.fy(tyf)+S.u(6)],fill=(120,96,70),rad=4)
            r.d.rectangle([S.fx(0.24),S.fy(tyf)-S.u(12),S.fx(0.76),S.fy(tyf)-S.u(9)],fill=(70,90,120,255))
            r.d.rectangle([S.fx(0.24),S.fy(tyf)+S.u(9),S.fx(0.76),S.fy(tyf)+S.u(12)],fill=(70,90,120,255))
        r.walls(accent=(180,160,120)); return
    # 1x1 or 2x1 barracks — bunks + locker
    r.deck(tint=(120,120,130))
    blankets=[(70,90,150),(120,80,80),(80,120,90),(110,100,60)]
    if r.cw>=2:  # barracks: row of bunks
        n=4
        for i in range(n):
            bx=S.fx(0.08+i*0.22)
            _bunk(r,bx,S.fy(0.18),S.fx(0.16),S.fy(0.44),blankets[i%4])
        # lockers bottom
        for i in range(n):
            bx=S.fx(0.08+i*0.22)
            r.panel([bx,S.fy(0.70),bx+S.fx(0.16),S.fy(0.88)],fill=STEEL_D)
    else:  # single quarters
        _bunk(r,S.fx(0.12),S.fy(0.16),S.fx(0.40),S.fy(0.50),blankets[0])
        # locker + desk
        r.panel([S.fx(0.62),S.fy(0.16),S.fx(0.86),S.fy(0.52)],fill=STEEL)
        for gy in (0.24,0.34,0.44):
            r.d.line([(S.fx(0.62),S.fy(gy)),(S.fx(0.86),S.fy(gy))],fill=A(EDGE,150),width=SS)
        r.panel([S.fx(0.14),S.fy(0.72),S.fx(0.54),S.fy(0.86)],fill=(120,96,70))  # desk
        r.screen([S.fx(0.60),S.fy(0.68),S.fx(0.86),S.fy(0.84)],col=AMBER)
    r.walls(accent=(150,150,170))

def medical_bay(r):
    # port of the approved ward; SurgicalBay (3x2) = operating room
    S=r; x0,y0,x1,y1=r.box()
    r.deck(tint=(150,168,178))
    if r.cw>=3 and r.ch>=2:  # SURGICAL BAY 3x2 — operating theatre
        r.d.rectangle([x0+S.u(3),S.fy(0.60),x1-S.u(3),S.fy(0.60)+S.u(3)],fill=A(RED,110))  # wayfinding
        # central operating table
        cx,cy=S.fx(0.5),S.fy(0.46)
        r.shadow([cx-S.u(14),cy-S.u(26),cx+S.u(14),cy+S.u(26)])
        r.panel([cx-S.u(14),cy-S.u(26),cx+S.u(14),cy+S.u(26)],fill=STEEL_L,rad=6,shadow=False)
        r.d.rounded_rectangle([cx-S.u(11),cy-S.u(23),cx+S.u(11),cy+S.u(23)],radius=S.u(4),fill=(210,225,235,255))
        r.d.ellipse([cx-S.u(6),cy-S.u(20),cx+S.u(6),cy-S.u(8)],fill=SKIN+(255,))  # patient
        # overhead surgical light ring
        for rr,al in [(S.u(20),60),(S.u(14),90),(S.u(8),150)]:
            r.d.ellipse([cx-rr,cy-rr,cx+rr,cy+rr],outline=(255,255,240,al),width=SS)
        r.d.ellipse([cx-S.u(3),cy-S.u(3),cx+S.u(3),cy+S.u(3)],fill=(255,255,235,255))
        # vitals monitors + instrument trays flanking
        _wall_monitor(r,S.fx(0.14),S.fy(0.24),S.u(20),S.u(14))
        _wall_monitor(r,S.fx(0.86),S.fy(0.24),S.u(20),S.u(14))
        for fx in (0.14,0.86):
            r.panel([S.fx(fx)-S.u(10),S.fy(0.62),S.fx(fx)+S.u(10),S.fy(0.80)],fill=STEEL)  # instrument tray
            for i in range(3):
                r.d.line([(S.fx(fx)-S.u(7)+i*S.u(6),S.fy(0.66)),(S.fx(fx)-S.u(7)+i*S.u(6),S.fy(0.76))],fill=STEEL_L+(255,),width=SS)
        _cabinet(r,S.fx(0.5)-S.u(10),S.fy(0.78),S.u(20),S.u(16))
        r.walls(accent=RED); return
    # 1x1 hospital ward
    r.d.rectangle([x0+S.u(3),S.fy(0.60),x1-S.u(3),S.fy(0.60)+S.u(3)],fill=A(RED,110))
    _hospital_bed(r,S.fx(0.11),S.fy(0.10),S.fx(0.30),S.fy(0.42))
    _hospital_bed(r,S.fx(0.59),S.fy(0.10),S.fx(0.30),S.fy(0.42))
    _wall_monitor(r,S.fx(0.26),S.fy(0.135),S.u(20),S.u(11))
    _wall_monitor(r,S.fx(0.74),S.fy(0.135),S.u(20),S.u(11))
    _iv_pole(r,S.fx(0.50),S.fy(0.30),S.u(34))
    _cabinet(r,S.fx(0.12),S.fy(0.70),S.u(22),S.u(20))
    _crash_cart(r,S.fx(0.68),S.fy(0.70),S.u(22),S.u(20))
    _wall_monitor(r,S.fx(0.50),S.fy(0.82),S.u(22),S.u(14))
    r.walls(accent=RED)

# medbay furniture helpers (ported)
BLANKET=(74,158,158); BLANKET_L=(104,186,184); BLANKET_D=(50,120,122); ECG=(96,214,180)
def _hospital_bed(r,x,y,w,h):
    S=r; d=r.d
    r.shadow([x,y,x+w,y+h])
    d.rounded_rectangle([x,y,x+w,y+h],radius=S.u(4),fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS*2)
    rail=S.u(3)
    d.rounded_rectangle([x+S.u(2),y+h*0.18,x+S.u(2)+rail,y+h*0.92],radius=rail,fill=STEEL_L+(255,))
    d.rounded_rectangle([x+w-S.u(2)-rail,y+h*0.18,x+w-S.u(2),y+h*0.92],radius=rail,fill=STEEL_L+(255,))
    mx0,my0,mx1,my1=x+rail+S.u(3),y+S.u(3),x+w-rail-S.u(3),y+h-S.u(3)
    d.rounded_rectangle([mx0,my0,mx1,my1],radius=S.u(4),fill=LINEN+(255,))
    d.rounded_rectangle([mx0+S.u(2),my0+S.u(2),mx1-S.u(2),my0+(my1-my0)*0.26],radius=S.u(4),fill=PILLOW+(255,))
    by0=my0+(my1-my0)*0.42
    d.rounded_rectangle([mx0+S.u(1),by0,mx1-S.u(1),my1-S.u(1)],radius=S.u(4),fill=BLANKET+(255,))
    d.rectangle([mx0+S.u(1),by0,mx1-S.u(1),by0+S.u(3)],fill=BLANKET_L+(255,))
    hx=(mx0+mx1)/2; hy=my0+(my1-my0)*0.20; hr=(mx1-mx0)*0.16
    d.ellipse([hx-hr,hy-hr,hx+hr,hy+hr],fill=SKIN+(255,),outline=A((150,120,100),180),width=SS)
def _wall_monitor(r,cx,cy,w,h):
    S=r; d=r.d
    d.rounded_rectangle([cx-w/2,cy-h/2,cx+w/2,cy+h/2],radius=S.u(3),fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS*2)
    r.screen([cx-w/2+S.u(3),cy-h/2+S.u(3),cx+w/2-S.u(3),cy+h/2-S.u(6)],col=ECG)
    for i,c in enumerate([LED_G,LED_A]):
        d.ellipse([cx-w/2+S.u(4)+i*S.u(7),cy+h/2-S.u(4),cx-w/2+S.u(8)+i*S.u(7),cy+h/2],fill=c+(255,))
def _iv_pole(r,cx,cy,h):
    S=r; d=r.d
    d.ellipse([cx-S.u(6),cy+h/2-S.u(3),cx+S.u(6),cy+h/2+S.u(3)],fill=STEEL_D+(255,),outline=EDGE+(200,),width=SS)
    d.line([(cx,cy-h/2),(cx,cy+h/2)],fill=STEEL_L+(255,),width=SS*2)
    d.line([(cx,cy-h/2),(cx+S.u(5),cy-h/2)],fill=STEEL_L+(255,),width=SS*2)
    bx=cx+S.u(3)
    d.rounded_rectangle([bx,cy-h/2+S.u(2),bx+S.u(8),cy-h/2+S.u(16)],radius=S.u(2),fill=(196,214,208,235),outline=STEEL_D+(200,),width=SS)
    d.rounded_rectangle([bx+SS,cy-h/2+S.u(8),bx+S.u(7),cy-h/2+S.u(15)],radius=SS,fill=(150,196,206,235))
def _cabinet(r,x,y,w,h):
    S=r; d=r.d
    d.rounded_rectangle([x,y,x+w,y+h],radius=S.u(3),fill=STEEL+(255,),outline=EDGE+(255,),width=SS*2)
    d.line([((x+x+w)/2,y+S.u(2)),((x+x+w)/2,y+h-S.u(2))],fill=EDGE+(200,),width=SS)
    d.rounded_rectangle([x+S.u(2),y+S.u(2),x+w-S.u(2),y+h*0.5],radius=S.u(2),fill=A(STEEL_L,60))
    cx,cy=(x+x+w)/2,y+h*0.72; s=min(w,h)*0.14
    d.rectangle([cx-s*0.33,cy-s,cx+s*0.33,cy+s],fill=RED+(255,))
    d.rectangle([cx-s,cy-s*0.33,cx+s,cy+s*0.33],fill=RED+(255,))
def _crash_cart(r,x,y,w,h):
    S=r; d=r.d
    d.rounded_rectangle([x,y,x+w,y+h],radius=S.u(3),fill=(196,74,74,255),outline=EDGE+(255,),width=SS*2)
    for i in range(3):
        dy=y+S.u(3)+i*(h-S.u(6))/3
        d.rounded_rectangle([x+S.u(2),dy,x+w-S.u(2),dy+(h-S.u(6))/3-S.u(2)],radius=S.u(2),fill=A(RED_D,220))

# ============================================================ UTILITY
def navigation(r):
    r.deck(tint=(110,130,160))
    S=r; x0,y0,x1,y1=r.box()
    if r.cw>=3:  # BridgeWing 3x2 — full bridge
        # forward viewscreen (top)
        r.panel([S.fx(0.12),S.fy(0.10),S.fx(0.88),S.fy(0.14)],fill=STEEL_D)
        r.screen([S.fx(0.14),S.fy(0.12),S.fx(0.86),S.fy(0.30)],col=ENERGY,grid=True)
        # captain chair center
        cx,cy=S.fx(0.5),S.fy(0.62)
        r.panel([cx-S.u(10),cy-S.u(12),cx+S.u(10),cy+S.u(12)],fill=(60,80,120),rad=6)
        r.d.rounded_rectangle([cx-S.u(7),cy-S.u(8),cx+S.u(7),cy+S.u(10)],radius=S.u(4),fill=(80,100,140,255))
        # flanking helm consoles
        for fx in (0.20,0.80):
            r.panel([S.fx(fx)-S.u(12),S.fy(0.52),S.fx(fx)+S.u(12),S.fy(0.66)],fill=STEEL)
            r.screen([S.fx(fx)-S.u(9),S.fy(0.54),S.fx(fx)+S.u(9),S.fy(0.64)],col=CYAN)
            r.panel([S.fx(fx)-S.u(6),S.fy(0.72),S.fx(fx)+S.u(6),S.fy(0.84)],fill=(60,80,120),rad=6)  # seat
        r.walls(accent=ENERGY); return
    if r.cw==2:  # AICombatCore 2x1 — command/AI console
        # server racks
        for fx in (0.16,0.30):
            r.panel([S.fx(fx)-S.u(7),S.fy(0.18),S.fx(fx)+S.u(7),S.fy(0.72)],fill=STEEL_D)
            for gy in range(6):
                r.d.ellipse([S.fx(fx)-S.u(4),S.fy(0.24+gy*0.08),S.fx(fx)-S.u(1),S.fy(0.27+gy*0.08)],fill=LED_G+(255,))
        # big tactical screen
        r.screen([S.fx(0.46),S.fy(0.20),S.fx(0.90),S.fy(0.66)],col=ENERGY,grid=True)
        r.panel([S.fx(0.60),S.fy(0.74),S.fx(0.80),S.fy(0.86)],fill=(60,80,120),rad=6)  # seat
        r.walls(accent=ENERGY); return
    # 1x1 helm
    r.screen([S.fx(0.16),S.fy(0.18),S.fx(0.84),S.fy(0.48)],col=ENERGY,grid=True)  # nav chart
    r.panel([S.fx(0.30),S.fy(0.52),S.fx(0.70),S.fy(0.64)],fill=STEEL)  # console
    for i in range(3):
        r.d.ellipse([S.fx(0.36+i*0.12),S.fy(0.56),S.fx(0.40+i*0.12),S.fy(0.60)],fill=[LED_G,LED_A,ENERGY][i]+(255,))
    r.panel([S.fx(0.38),S.fy(0.70),S.fx(0.62),S.fy(0.86)],fill=(60,80,120),rad=8)  # pilot seat
    r.d.rounded_rectangle([S.fx(0.41),S.fy(0.72),S.fx(0.59),S.fy(0.84)],radius=S.u(4),fill=(80,100,140,255))
    r.walls(accent=ENERGY)

def repair_station(r):
    r.deck(tint=(150,140,110))
    S=r
    big=r.cw>=2
    # workbench along top
    r.panel([S.fx(0.10),S.fy(0.16),S.fx(0.90 if not big else 0.60),S.fy(0.30)],fill=STEEL)
    # tool rack (wall) with hanging tools
    for i in range(5):
        tx=S.fx(0.16+i*0.13)
        r.d.line([(tx,S.fy(0.10)),(tx,S.fy(0.15))],fill=STEEL_D+(255,),width=S.u(2))
        r.d.rectangle([tx-S.u(2),S.fy(0.10),tx+S.u(2),S.fy(0.14)],fill=[AMBER,STEEL_L,AMBER,STEEL_L,AMBER][i]+(255,))
    # spare parts crate
    r.panel([S.fx(0.14),S.fy(0.62),S.fx(0.40),S.fy(0.84)],fill=STEEL_D)
    r.d.line([(S.fx(0.27),S.fy(0.62)),(S.fx(0.27),S.fy(0.84))],fill=A(AMBER_D,160),width=SS)
    if big:  # drone bay: charging pad + drone
        px,py=S.fx(0.74),S.fy(0.5)
        r.d.ellipse([px-S.u(20),py-S.u(16),px+S.u(20),py+S.u(16)],outline=A(ENERGY,120),width=S.u(2))
        r.panel([px-S.u(10),py-S.u(8),px+S.u(10),py+S.u(8)],fill=STEEL_D,rad=4)  # drone body
        for (ox,oy) in [(-S.u(12),-S.u(10)),(S.u(12),-S.u(10)),(-S.u(12),S.u(10)),(S.u(12),S.u(10))]:
            r.d.ellipse([px+ox-S.u(4),py+oy-S.u(4),px+ox+S.u(4),py+oy+S.u(4)],fill=STEEL+(255,),outline=EDGE+(255,),width=SS)  # rotors
        r.d.ellipse([px-S.u(3),py-S.u(3),px+S.u(3),py+S.u(3)],fill=ENERGY+(255,))
    else:
        # robotic arm
        bx,by=S.fx(0.70),S.fy(0.72)
        r.d.ellipse([bx-S.u(8),by-S.u(5),bx+S.u(8),by+S.u(6)],fill=STEEL_D+(255,),outline=EDGE+(255,),width=SS)
        j2=(S.fx(0.62),S.fy(0.50))
        r.d.line([(bx,by),j2],fill=STEEL_L+(255,),width=S.u(5))
        r.d.line([j2,(S.fx(0.78),S.fy(0.40))],fill=STEEL_L+(255,),width=S.u(4))
        r.d.ellipse([j2[0]-S.u(3),j2[1]-S.u(3),j2[0]+S.u(3),j2[1]+S.u(3)],fill=STEEL+(255,))
    r.walls(accent=AMBER)

def floodlight(r):
    r.deck(tint=(150,150,130))
    S=r; cx,cy=S.fx(0.5),S.fy(0.42)
    # beam glow downward
    g=Image.new("RGBA",r.img.size,(0,0,0,0)); gd=ImageDraw.Draw(g)
    gd.polygon([(cx-S.u(6),cy),(cx+S.u(6),cy),(cx+S.u(26),S.fy(0.92)),(cx-S.u(26),S.fy(0.92))],fill=(255,240,190,60))
    r.img.alpha_composite(blur(g,3)); r.d=ImageDraw.Draw(r.img)
    # lamp housing on yoke
    r.panel([cx-S.u(8),cy-S.u(16),cx+S.u(8),cy-S.u(4)],fill=STEEL_D,rad=3)  # mount
    r.d.ellipse([cx-S.u(16),cy-S.u(10),cx+S.u(16),cy+S.u(16)],fill=STEEL+(255,),outline=EDGE+(255,),width=S.u(2))
    r.d.ellipse([cx-S.u(12),cy-S.u(6),cx+S.u(12),cy+S.u(12)],fill=(255,248,220,255))
    r.d.ellipse([cx-S.u(6),cy-S.u(1),cx+S.u(6),cy+S.u(7)],fill=(255,255,245,255))
    r.walls(accent=(230,210,150))

def docking_port(r):
    r.deck(tint=(110,120,135))
    S=r; cx,cy=S.fx(0.5),S.fy(0.5)
    R=min(S.W,S.H)*0.34
    r.hazard_edge(True,True) if r.cw>=3 else None
    # outer clamp ring
    r.d.ellipse([cx-R,cy-R,cx+R,cy+R],fill=STEEL_D+(255,),outline=EDGE+(255,),width=S.u(3))
    r.d.ellipse([cx-R+S.u(4),cy-R+S.u(4),cx+R-S.u(4),cy+R-S.u(4)],fill=DARK+(255,))
    # locking clamps
    for a in range(8):
        ang=a*math.pi/4
        ex,ey=cx+math.cos(ang)*R*0.9,cy+math.sin(ang)*R*0.9
        ix,iy=cx+math.cos(ang)*R*0.6,cy+math.sin(ang)*R*0.6
        r.d.line([(ix,iy),(ex,ey)],fill=STEEL_L+(255,),width=S.u(4))
    # inner hatch iris
    r.d.ellipse([cx-R*0.5,cy-R*0.5,cx+R*0.5,cy+R*0.5],fill=STEEL+(255,),outline=EDGE+(255,),width=S.u(2))
    for a in range(6):
        ang=a*math.pi/3
        r.d.line([(cx,cy),(cx+math.cos(ang)*R*0.5,cy+math.sin(ang)*R*0.5)],fill=A(EDGE,150),width=SS)
    r.d.ellipse([cx-S.u(4),cy-S.u(4),cx+S.u(4),cy+S.u(4)],fill=CAUTION+(255,))
    r.walls(accent=CAUTION)

def hull_beam(r):
    # "most basic block ever" — plain armored structural plate
    S=r; x0,y0,x1,y1=r.box()
    r.d.rounded_rectangle(r.box(),radius=S.u(6),fill=mix(STEEL,DARK,0.35)+(255,))
    # brushed plating seams per cell
    cw=(x1-x0)/r.cw; ch=(y1-y0)/r.ch
    for i in range(1,r.cw):
        r.d.line([(x0+i*cw,y0+S.u(4)),(x0+i*cw,y1-S.u(4))],fill=A(EDGE,180),width=S.u(2))
    for j in range(1,r.ch):
        r.d.line([(x0+S.u(4),y0+j*ch),(x1-S.u(4),y0+j*ch)],fill=A(EDGE,180),width=S.u(2))
    # corner rivets per cell
    for i in range(r.cw):
        for j in range(r.ch):
            for (fx,fy) in [(0.18,0.18),(0.82,0.18),(0.18,0.82),(0.82,0.82)]:
                bx=x0+(i+fx)*cw; by=y0+(j+fy)*ch
                r.d.ellipse([bx-S.u(2),by-S.u(2),bx+S.u(2),by+S.u(2)],fill=STEEL_D+(255,))
                r.d.ellipse([bx-S.u(1),by-S.u(1),bx+S.u(0.5),by+S.u(0.5)],fill=STEEL_L+(200,))
    # subtle highlight
    r.d.rounded_rectangle([x0+S.u(3),y0+S.u(3),x1-S.u(3),y1-S.u(3)],radius=S.u(5),outline=A(STEEL_L,50),width=SS)
    r.walls()

# ============================================================ JOBS
BUILDERS={
 'small_reactor':small_reactor,'large_reactor':large_reactor,'battery':battery,
 'standard_engine':standard_engine,'silent_drive':silent_drive,
 'oxygen_scrubber':oxygen_scrubber,'life_support':life_support,
 'point_defense':point_defense,'railgun':railgun,'torpedo_tube':torpedo_tube,
 'mine_layer':mine_layer,'salvage_arm':salvage_arm,
 'sonar_array':sonar_array,'passive_sonar':passive_sonar,'depth_sensor':depth_sensor,
 'cargo_hold':cargo_hold,'ballast_tank':ballast_tank,'research_lab':research_lab,
 'basic_quarters':basic_quarters,'medical_bay':medical_bay,
 'navigation':navigation,'repair_station':repair_station,'floodlight':floodlight,
 'docking_port':docking_port,'hull_beam':hull_beam,
}
JOBS={
 'small_reactor':[(1,1)],'large_reactor':[(2,1),(3,3)],'battery':[(1,1)],
 'standard_engine':[(1,1),(2,1)],'silent_drive':[(1,1)],
 'oxygen_scrubber':[(1,1),(2,1)],'life_support':[(1,1)],
 'navigation':[(1,1),(2,1),(3,2)],'point_defense':[(1,1)],'railgun':[(1,1),(2,1)],
 'torpedo_tube':[(1,1)],'mine_layer':[(1,1)],'sonar_array':[(1,1)],
 'passive_sonar':[(1,1),(2,1)],'depth_sensor':[(1,1)],
 'cargo_hold':[(1,1),(2,1),(2,2)],'ballast_tank':[(1,1)],'research_lab':[(1,1),(2,1)],
 'basic_quarters':[(1,1),(2,1),(2,2),(3,3)],'medical_bay':[(1,1),(3,2)],
 'repair_station':[(1,1),(2,1)],'floodlight':[(1,1)],
 'docking_port':[(1,1),(3,3)],'salvage_arm':[(1,1)],
 'hull_beam':[(1,1),(2,2),(3,2)],
}

def fname(base,cw,ch):
    if (cw,ch)==(1,1): return f"{base}.png"
    return f"{base}_{cw}x{ch}.png"

if __name__=="__main__":
    import sys
    only=sys.argv[1] if len(sys.argv)>1 else None
    made=[]
    for base,foots in JOBS.items():
        if only and base!=only: continue
        for (cw,ch) in foots:
            r=Room(cw,ch)
            BUILDERS[base](r)
            sz=r.save(fname(base,cw,ch))
            made.append((fname(base,cw,ch),sz)); print("·",fname(base,cw,ch),sz)
    print(f"\n{len(made)} sprites generated")
