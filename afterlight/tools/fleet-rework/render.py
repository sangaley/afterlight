#!/usr/bin/env python3
"""Portraits of designs for the fleet review site, rendered by the game itself.

For each design JSON the game is launched on its own (muted, offscreen at
2560x1440, empty space keyed pure green, the camera exactly on the ship), with
the design flown as your ship (AFTERLIGHT_DESIGN, so it wears its own paint).
Two shots:

  <key>_shell.png   the painted shell, at a zoom where the roof is fully on
  <key>_inside.png  the lit interior: '.' takes the roof off and the camera
                    eases in until the ship fills the frame

Both are cropped to the ship and keyed onto transparency. Nothing is added to
the game: the designs are read from wherever they are.

    python3 tools/fleet-rework/render.py <out_dir> <design.json> [...]

Run from afterlight/ after `cargo build` (it runs target/debug/afterlight).
"""
import json, os, signal, subprocess, sys, time

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools", "faction-arming"))
import kit  # noqa: E402  (module footprints from the registry)

W, H = 2560, 1440
CELL = 66.0
# Roof fully on from 5.5 (vfx::ship_roof); the far picture starts at 8.
SHELL_ZOOM = (5.6, 7.9)
# Real seconds after launch: the game takes 10-14 s to come up on a busy
# machine. The clock stops then (a parked ship slowly turns), and the shot
# waits for the camera to settle.
FREEZE, SHOT = 16.0, 18.0


def extent(design):
    """Ship-local bounds of every block, in game units: (x0, x1, y0, y1)."""
    cells = [tuple(c["grid_pos"]) for c in design["hull_cells"]]
    for m in design["modules"]:
        if (m.get("extras") or {}).get("turret"):
            continue
        cells += kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m.get("rotation", "North"))
    xs = [c[0] for c in cells]
    ys = [c[1] for c in cells]
    return (min(xs) * CELL - CELL, (max(xs) + 1) * CELL, min(ys) * CELL - CELL, (max(ys) + 1) * CELL)


def zooms(design):
    """(shell zoom, inside zoom, frame size). A ship too big for the usual
    frame at a zoom that still draws its blocks gets a bigger frame."""
    x0, x1, y0, y1 = extent(design)
    # The camera sits on the ship's origin, not its middle: fit the far side.
    w, h = 2 * max(-x0, x1), 2 * max(-y0, y1)
    fw = min(4096, max(W, int(w / (SHELL_ZOOM[1] * 0.95 * 0.85)) + 16))
    fh = min(4096, max(H, int(h / (SHELL_ZOOM[1] * 0.95 * 0.85)) + 16))
    fit = max(w / (fw * 0.85), h / (fh * 0.85))
    shell = min(max(fit, SHELL_ZOOM[0]), SHELL_ZOOM[1])
    inside = min(max(fit, 1.2), SHELL_ZOOM[1])
    return shell, inside, (fw, fh)


def keyed_crop(path, out, ext, zoom):
    """Key the green background away (with its spill on the hull's edges),
    then crop to the ship. The ship is alone in the frame (AFTERLIGHT_PORTRAIT)."""
    import numpy as np
    a = np.asarray(Image.open(path).convert("RGBA")).astype(np.int16)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    other = np.maximum(r, b)
    # Pure background, and edge pixels mostly made of it.
    background = (g > 120) & (g - other > 60)
    a[..., 3] = np.where(background, 0, 255)
    # What's left keeps no green cast: the edges were blended with the key.
    a[..., 1] = np.where(g > other + 12, other + 12, g)
    im = Image.fromarray(a.clip(0, 255).astype(np.uint8), "RGBA")
    box = im.getbbox()
    if box:
        w, h = im.size
        im = im.crop((max(0, box[0] - 8), max(0, box[1] - 8), min(w, box[2] + 8), min(h, box[3] + 8)))
    im.save(out)
    return im.size


def shoot(design_path, zoom, inside, shots, frame=(W, H)):
    """One run of the game: the design at a fixed zoom, the clock stopped once
    it's up (a parked ship slowly turns), and one shot on cue with F7. With
    `inside`, '.' takes the roof off first."""
    os.makedirs(shots, exist_ok=True)
    for f in os.listdir(shots):
        os.remove(os.path.join(shots, f))
    env = dict(os.environ)
    # cargo run's library path: the game links Bevy as a shared library.
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True).stdout.strip()
    libs = [os.path.join(ROOT, "target", "debug", "deps"), os.path.join(ROOT, "target", "debug"), os.path.join(sysroot, "lib"),
            os.path.join(sysroot, "lib", "rustlib", "aarch64-apple-darwin", "lib")]
    keys = f"{FREEZE - 1.0}:Period," if inside else ""
    env.update({
        "DYLD_FALLBACK_LIBRARY_PATH": ":".join(libs + [env.get("DYLD_FALLBACK_LIBRARY_PATH", "")]),
        "CARGO_MANIFEST_DIR": ROOT,
        "AFTERLIGHT_MUTE": "1",
        "AFTERLIGHT_SKIP_MENU": "1",
        "AFTERLIGHT_MOVETEST": "1",
        "AFTERLIGHT_DESIGN": os.path.abspath(design_path),
        "AFTERLIGHT_OFFSCREEN": "1",
        "AFTERLIGHT_OFFSCREEN_SIZE": f"{frame[0]}x{frame[1]}",
        "AFTERLIGHT_HIDE_UI": "1",
        "AFTERLIGHT_KEY_BG": "1",
        "AFTERLIGHT_PORTRAIT": "1",
        "AFTERLIGHT_CAM_AT": "ship",
        "AFTERLIGHT_ZOOM": str(zoom),
        "AFTERLIGHT_FREEZE_AT": str(FREEZE),
        "AFTERLIGHT_KEYS": f"{keys}{SHOT}:F7",
        "AFTERLIGHT_SHOTS_DIR": shots,
    })
    log = open(os.path.join(shots, "run.log"), "w")
    proc = subprocess.Popen([os.path.join(ROOT, "target", "debug", "afterlight")], cwd=ROOT, env=env,
                            stdout=log, stderr=subprocess.STDOUT)
    try:
        deadline = time.time() + SHOT + 30
        while time.time() < deadline:
            if any(f.endswith(".png") for f in os.listdir(shots)) or proc.poll() is not None:
                break
            time.sleep(0.5)
        time.sleep(1.0)
    finally:
        if proc.poll() is None:
            proc.send_signal(signal.SIGTERM)
            try:
                proc.wait(10)
            except subprocess.TimeoutExpired:
                proc.kill()
    done = sorted(f for f in os.listdir(shots) if f.endswith(".png"))
    return os.path.join(shots, done[0]) if done else None


def render(design_path, out_dir):
    design = json.load(open(design_path))
    key = os.path.splitext(os.path.basename(design_path))[0]
    shell, inside, frame = zooms(design)
    ext = extent(design)
    out = {}
    for name, zoom, roof_off in (("shell", shell, False), ("inside", inside, True)):
        shot = shoot(design_path, zoom, roof_off, os.path.join(out_dir, "_shots", f"{key}_{name}"), frame)
        if not shot:
            print(f"{key}: no {name} shot (see {out_dir}/_shots/{key}_{name}/run.log)")
            return None
        out[name] = keyed_crop(shot, os.path.join(out_dir, f"{key}_{name}.png"), ext, zoom)
    print(f"{key}: shell {out['shell'][0]}x{out['shell'][1]} at zoom {shell:.2f}, inside {out['inside'][0]}x{out['inside'][1]} at zoom {inside:.2f}")
    meta = {"key": key, "shell_zoom": shell, "inside_zoom": inside}
    # Game units per pixel of each portrait, for drawing them to one scale.
    json.dump(meta, open(os.path.join(out_dir, f"{key}.meta.json"), "w"))
    return meta


def main():
    out_dir = sys.argv[1]
    os.makedirs(out_dir, exist_ok=True)
    for path in sys.argv[2:]:
        render(path, out_dir)


if __name__ == "__main__":
    main()
