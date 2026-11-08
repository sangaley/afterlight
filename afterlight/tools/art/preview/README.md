# Art previews

Candidate sprites rendered by `tools/art/blender/`. **Nothing here is live** --
the game still loads everything from `assets/sprites/`. These are proposals.

| File | What it is |
|---|---|
| `railgun_CURRENT_in_game.png` | What the game renders today (base + barrel) |
| `railgun_side_by_side.png` | Current vs proposed, same scale |
| `railgun_at_game_size.png` | Both at real combat zoom (66px, nearest-neighbour 4x) |
| `railgun_assembled.png` | Proposed base + barrel composited |
| `railgun_base.png` | Proposed base plate alone (378x378) |
| `railgun_barrel.png` | Proposed barrel alone (300x300, spans 2 cells) |

Open the whole folder and press space to flip through with Quick Look:

    open afterlight/tools/art/preview/

Regenerate the module contact sheet (all 45 live sprites):

    python3 tools/art/contact_sheet.py && open tools/art/modules_sheet.png

Re-render a weapon candidate:

    /Applications/Blender.app/Contents/MacOS/Blender -b \
        -P tools/art/blender/weapons.py -- railgun /tmp/out.png 1024
