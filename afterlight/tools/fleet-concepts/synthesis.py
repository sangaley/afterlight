"""Synthesis Collective: flesh and machine, merged.

Lore: a merger of meat and network that calls itself one mind. "The screaming
in the networks is just data corruption." Self-healing and fanatical: they
rarely flee, ram as a kamikaze below 20% hull, and aim for your reactor.

Grammar, held at every size (all of it reads from above):
  - ORGANIC AND LOPSIDED. Bulbous lobes built from rounded staircases, no
    straight edge longer than a few cells. Asymmetry is the norm, never
    a mirror.
  - TENDRILS three cells thick (rim, vein, rim) that curl and grasp; their
    tips carry the guns and the only armour plates.
  - GRAFTS: mismatched sections fused at odd angles, a round flesh hull on a
    long segmented one, lobes joined by short tendrils round open voids.
  - Inside: veins of deck branching through every lobe and down every
    tendril, organs packed along them. Repair bays everywhere.
  - Composite. Plasma casters, ion disruptors, tractor beams and salvage
    arms to hold what they catch.

Every ship below is drawn cell by cell, x0/y_top given per block. Legend:
draw.py. P plasma, i ion, y repair bay, % advanced repair bay, Z creature
containment, Y specimen vault, @ AI combat core, w wellness hub, t tractor
beam, X salvage arm, J signal buoy, ! research lab, h docking hub.
"""
from draw import Canvas

FACTION = {
    "slug": "synthesis_collective",
    "name": "Synthesis Collective",
    "tagline": "A flesh-machine merger that calls itself one mind. The screaming in the networks is just data corruption.",
    "grammar": [
        "Organic and lopsided: bulbous lobes of rounded staircases, no straight edge longer than a few cells",
        "Tendrils three cells thick that curl and grasp, guns and the only plates on their tips",
        "Mismatched sections grafted together at odd angles, lobes joined round open voids",
        "Veins of deck branching through every lobe and tendril, organs packed along them",
        "Composite; plasma, ion disruptors, tractor beams, and repair bays everywhere",
    ],
    "accent": "#b26bd6",
}


def _draw(name, x0, y_top, block):
    c = Canvas(name, "Composite")
    c.rows(x0, y_top, block)
    return c.build()


def graft():
    """Corvette. A round flesh bulb with a grafted engine lobe on its upper
    quarter, a knot aft-low, and one hooked tendril curling down in front of
    the bow with plasma on its plated tip."""
    return _draw("sy_graft", -10, 6, """
 ###
#Q.y#  #i#i#  ##P##P##/
E..MO##y.M.y##........i
E....d..........y#P#y.P
#RbQ.SSS.Oy.m.y.#   #.i
 ##..SSS.H...Q.P    #.#
   E.y@@..Rb.%%#    #.#
   #.yfc.ZZ.O..i    #.#
  #........yM.#     /P/
 E.y.O.Y.V.yb.i
 E.Q.y#%%.y.##
 #y.y# #P#i#
  ###
""")


def chimera():
    """Frigate. Two different hulls fused off-axis: a round flesh hull
    aft (quarters, surgery, containment, the bridge) grafted onto a long
    segmented hull that runs down and forward at an angle (fusion core, AI
    core, the guns), with a tendril arching over the top."""
    return _draw("sy_chimera", -17, 14, """
                  #i#i#
              ####.....##
             #..y..y.M..y#
         ### #.Q.#####.y.OP
       ##VV.#..y#     #..yi
     ##Q.Q.ZZ.m#       #..#
     #........O#       /#P/
    #ZZ.SSS.GGyQ#
    #...SSS.GGb.P
   E.Rf.BBB.M.y.Oi
   E....BBB.....y#
   E.Rb.KK..Yy.V.i
    #Q.O.y..y.ZZ.#
  ##ybc.@@..%%.y..#i##
 E...y...R.b....O...y.P
E.Q.O.M.y..y..FFF.M.%%.#i##
E.Rc.y.##.y.b.FFF......y.T.P
#y.Q.y#  #i##.FFF.Q.cb..@@..#P#
 E.y.#       #P#i..y.O.M..y.b.yi
  ###            #i##.y.R..y..O.#P#
                     #..by.M.T....yi
                      #i#.SSS.Q.y.b.#/
                         #SSS.y.M.y..##
                          #i#.y...R.y.P/
                             #y.O.b..T.i
                              #P#..y.yi/
                                 ##P##/
""")


def assimilator():
    """Capital. A lobed body with two vacuoles, shoulders, and two long
    grasping tendrils plus a thumb round an open cavity big enough to hold a
    captured ship. Every face that looks into the cavity is a tractor beam
    or a salvage arm."""
    return _draw("sy_assimilator", -27, 16, """
                                    ##i##P##i##P#
                                  ##.......y.y...i
                           #i# #P#..y..Mt#X#t#.y..P/
                         ##..y#..Q.y.#t       X.y..#/
      ###               #.y.M.%%..#i#          X.y..i
    ##.y.##     #P#i#P# #y.SSS.y.t              X.y.#
   #.Rc.CC.#  ##y.Q.Q.y#...SSS.O.#               t.b.P
   #.bf.CC.M##.........d.y.....y.t                X..#
  E.R.LL.O.GG.y.m.Q..y.!!.ZZ....y#                #.yi
  E.bc.f.y.GG.y###.y.Y.VV.SSShhht                  t.#
  E.R.FFF.ZZ.y#   #y.Y....SSShhht                  #.i
   E.yFFF.M.KK#   #..ym.M.y..hhh                   /P/
   E.RFFF.y.yO#   #Q.y..GG.yQi
  #....y.cb.y..###.y.BBBGG.O.y#  #i#P#i#
 #R.yQ...ZZ.y..%%..y.BBBH.y.!!.##..y.y..P
E.....d.......d...............d..........P
E.Ryb.Q.yyFFF.ywww.M.SSS.ZZ.y.y#i#t#X#.y.i
E.y.O.....FFF.ywww...SSS..y.M.#       #P#/
 #R.yQ...yFFF..www.y.....%%..y#
  #....d.............yQ.y.m.KKt                    /P/
   #.Rc.ZZ.VV.Y.SSS..###.y.O.y.t                  ##.#
   E.bf.yM.QQ.y.SSS.#   #.y.ZZ.yX                 t..i
   E.R.LL.y.CC.y.UU.#   #y......yt                X.y.P
  E.........CC.Q...y#   #@@.GG.M.X                 t..#
   E.Rc.ZZ.y.KK.VV.y.###..y.GG..yi                 X.y#
   #...d...............d.........P                 #b.i
   #.yO.yQ.My###y.Q.Q.y#.y.SSS.y.#                 t..#
    #.R.y.b.#   #i#P#i# #..SSS.O.i                X..#
     ##i#i##             #y.M.y..yP              t.y.i
                          #P#i#..y.#t           X..Q.#
                               #y..y.t        tX.y..P
                                ##.M..t#X#t  #..M.y#
                                  #.y....y.#t.y..##
                                   #i#P#..y..Q.yi
                                        #i#P#i##
""")


def bloom():
    """Experimental healer. Five teardrop petals round a heart, each petal
    a vein lined with repair bays and advanced repair bays. It sits behind the
    line and keeps the rest of the swarm alive; its bow petal is plated."""
    return _draw("sy_bloom", -15, 13, """
              #i#i#
             #y...y#
             #%%.%%#
             #yM.ym#
   ###       #%%.%%#
 ##y.y##     #y...y#
 #yy.%%#      #y.y#
E.Ry.y.O#     #O.y#
E....M%%#     #y.Q#     ##i##/
E.Ry.ycby### #yy.#     #y...y#
 #yy........#%%y.y# #i#%%.M.y#/
 ##yyy##..........y#..y.y...y.P
   ###  #yM.y@@.H...........yP/
         ##.SSS.BBB.ZZ.#y%%.y#
           #SSS.BBB.#i# ##i##/
          #.......y#
         #.yM.ym..Q#
        #y.y%%##y.%%#
       #%%.yy#  #.yO#
      E....y#   #..yM##
      ERy.My#    #.%%y.#
      E.y.%%#    #.y..y#
       #y.y#     #y%%.y#
        #i#      #y.y.y#
                 #y.M.y#
                  #i#i#
""")


def growth_node():
    """Station. An organic cluster of six lobes of different sizes, joined
    by short tendrils round open voids, one lobe curling back on itself.
    Fusion in four of them, the bridge in the central one."""
    return _draw("sy_growth_node", -23, 23, """
                   #i#P#
                 #iy.M.yi#
                 #.%%.%%.#
                #ySSS.ZZy.#
              ##..SSS.YY.yP
           #i#y..VV.m...Qy#
          #....M.ZZ.y.!!.yi
         #.y.y###y.O...GG.#
        #.y.y#   #y.y.QGG#
        i.My#    ##y...y##
       #.y.#       ##..#
      #.y.#          #.#             #i#
      P..y#          i.y#          ##...##
      #.y#           #.OP         #y.M.y.y#
   #i#...#           #.y#        #y.SSS.y.y#
  #y.y.Q.y#          #..y#       P..SSS.%%.i
 #y.%%.M.Oy#       #i.y.y.##    #y.Q.y.ZZ.y.o
#y.FFF.y.Q.y#    ##..y...yJ.i#  #.y.FFF.y.O.P
P.yFFF.ZZ.y.#    #y.@@.M.www.y##....FFF.M.y.#
#y.FFF.y.KK.#   #y.BBB.H.www......y.FFF.y.yi
oy.....Q.y.yO## #..BBB.y.www.y.Q.#.y.ZZ.!!.#
i.ybc.y.Y..VV.y#.y..m...y..O.%%.# #.y.y..y.#
#............d................yP   ##y.y#..#
P.y%%.M.yQ.y#i#y.SSS.y.FFF.y.Q.#     #i# #.#
 #y.y.O.y.y#   #ySSS.M.FFF.@@.y#        #..#
  #y.y.y.y#    #.......FFF.....i        P.y#
   #i#P#i#      #y.GG..y.ZZ.y.#         #..i
                P.yGG..VV..y.y#         #.#
                 #y.y..y.Q.%%.y#       #..P
                 ##.M..y.....y.y#     #.y.#
                   #.y.O.y#i#y..y#    #..y#
                  #y..#i##   #y...#  #.y.#
                 #..y#        #y...##..y#
                #.y..#        #..y.y.y..i
             #i#y.M.#         #y.SSS.y.#
            #..y.ZZ#          #..SSS.y#
           #y.FFF.y#         #y.y.M.y.y#
          #y..FFF.%%#         P.%%.ZZ.i
          P.y.FFF.y.i         #y.y.y.y#
          o.yMQ.%%..#         #..MQ..y#
          #.y.y.y.y.#          #i#P#i#
          #y.VV.YY.y#
           #y.m.Q.y#
            #.y.y.#
             #i#P#
""")


CONCEPTS = [
    ("graft", graft, "Graft", "Corvette",
     "A round flesh bulb with a grafted engine lobe and one hooked tendril curling in front of the bow, plasma on its plated tip."),
    ("chimera", chimera, "Chimera", "Frigate",
     "Two different hulls fused off-axis: a round flesh hull grafted onto a long segmented one running down and forward, a tendril arched over both."),
    ("bloom", bloom, "Bloom", "Experimental: healer",
     "Five teardrop petals round a heart, every petal lined with repair bays. It hangs behind the swarm and keeps it alive."),
    ("assimilator", assimilator, "Assimilator", "Capital",
     "A lobed body with two grasping tendrils and a thumb round an open cavity for a captured ship, lined with tractor beams and salvage arms."),
    ("growth_node", growth_node, "Growth Node", "Station",
     "Six lobes of different sizes joined by short tendrils round open voids, one curling back on itself. The Collective's nest."),
]
