# Afterlight — Install & Run

A step-by-step guide to getting the game building and running on a **Mac with Apple
Silicon** (M1/M2/M3/M4). It is written for someone who has never touched Rust — every
command is meant to be copied and pasted as-is.

There is no installer and no double-clickable app. You compile the game from source.
That sounds worse than it is: it's four commands, and only the first run is slow.

---

## Before you start

| What | Why |
|---|---|
| **~12 GB free disk space** | The source is 209 MB. The build cache is the other ~10 GB. |
| **20–30 minutes** | Almost entirely the first compile, which needs no supervision. |
| **A stable internet connection** | The first build downloads several hundred dependencies. |

Check your free space first — a full disk produces bizarre, misleading errors rather
than an honest "disk full":

```bash
df -h /
```

---

## 1. Install Xcode Command Line Tools

Rust needs Apple's linker to produce a binary. This is a ~1 GB download from Apple.

```bash
xcode-select --install
```

A dialog will appear — click **Install** and wait. If it says *"command line tools are
already installed"*, you're done with this step.

Verify:

```bash
xcode-select -p
# expect: /Library/Developer/CommandLineTools  (or an Xcode.app path)
```

---

## 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When it prompts you, press **1** for the standard installation.

**Then close your terminal and open a new one.** The installer adds Rust to your `PATH`,
but only for shells started afterwards. (If you'd rather not reopen it, run
`source "$HOME/.cargo/env"` instead.)

Verify:

```bash
rustc --version
cargo --version
```

Both should print a version number. Known-good: **1.96.1**. Anything newer is fine —
Rust is strongly backwards compatible.

---

## 3. Get the code

```bash
git clone https://github.com/sangaley/afterlight.git
cd afterlight/afterlight
```

**That doubled directory name is not a typo.** The repository is called `afterlight`
and the Rust package inside it is also called `afterlight`. You must be in the *inner*
one to build or run, because the game loads its sprites, audio and ship designs from
paths relative to wherever you started it. Run it from the outer directory and you'll get
a window full of missing textures.

Confirm you're in the right place — this should list a `Cargo.toml` and an `assets` folder:

```bash
ls
```

---

## 4. Build and run

```bash
cargo run
```

Now leave it alone.

**The first build takes 10–20 minutes.** The game is built on the Bevy engine, which is a
large dependency tree compiled from scratch the first time. What you'll see:

- A long list of `Compiling <something>` lines scrolling past — this is normal.
- Long silent pauses with no output at all, sometimes minutes — **this is also normal.**
  It is not frozen. If you want proof it's working, open Activity Monitor and you'll see
  `rustc` burning CPU.
- Warnings in yellow. Harmless. Only red `error:` lines actually stop the build.

When it finishes, the game window opens on its own.

**Every run after this one takes seconds**, not minutes. The slow part happens exactly
once, unless you run `cargo clean` or change the dependency list.

---

## 5. Playing it

Press **Enter** to get through the menus and launch.

The essentials to not die immediately:

| Key | Action |
|---|---|
| `WASD` | Fly the ship |
| `Q` / `E` | Vertical thrusters |
| `Space` | Fire weapons |
| `Z` | Radar ping — reveals what's around you |
| `B` | Build mode (place and remove modules) |
| `C` | Crew menu |
| `F` | Dock with a station / seal a bulkhead, depending on context |
| `Esc` | Pause |

The full control list is in [README.md](README.md), and
[afterlight/docs/GAME_DESIGN_DOC.MD](afterlight/docs/GAME_DESIGN_DOC.MD) explains
what the game is actually trying to be.

Your saves are written into the `afterlight/` folder you're running from, so don't
delete it between sessions.

---

## 6. If you want to change the code

You have read access to the repository but not write access, which is the normal
arrangement — you work on your own copy and offer changes back.

1. Click **Fork** at the top right of
   [github.com/sangaley/afterlight](https://github.com/sangaley/afterlight). You now
   have your own full copy.
2. Clone your fork instead of the original:

   ```bash
   git clone https://github.com/YOUR-USERNAME/afterlight.git
   cd afterlight/afterlight
   ```

3. Work on a branch, never on `master`:

   ```bash
   git checkout -b what-im-changing
   ```

4. Check your work compiles without doing a full build — this is much faster than
   `cargo run` and catches most mistakes:

   ```bash
   cargo check
   ```

5. Push the branch to your fork and open a **Pull Request** on GitHub. That's the request
   for the change to be pulled into the real repo.

The source lives in `afterlight/src/` — 166 files, organised by subject (`combat/`,
`crew/`, `building/`, `celestial/`). `afterlight/CLAUDE.md` is the working notes on how
the systems fit together and is the fastest way in.

> **On the licence:** this repo is public to *read*, but [LICENSE](LICENSE) reserves all
> rights — it does not by itself grant permission to modify or redistribute. You've been
> invited personally, so ask the author to confirm in writing what you're allowed to do
> with any changes you make. This matters less for a weekend of tinkering than for
> anything you'd publish.

---

## Troubleshooting

**`zsh: command not found: cargo`**
Your shell doesn't know about Rust yet. Open a new terminal window, or run
`source "$HOME/.cargo/env"`.

**`linker 'cc' not found`** or `error: linking with 'cc' failed`
Step 1 didn't complete. Run `xcode-select --install` again.

**`rustc interrupted by SIGSEGV`, or `failed to link or copy`, or the whole machine hangs**
This is almost always a **full disk**, not a broken compiler. Run `df -h /`. If you're
under a couple of GB, free some space and try again.

**The window opens but textures are missing, or it panics on startup**
You're in the wrong directory. You must be in the *inner* `afterlight/afterlight`,
not the repository root. Check with `ls` — you want to see `Cargo.toml` and `assets`.

**`dyld: Library not loaded: @rpath/libbevy_dylib...`**
You ran the compiled binary directly out of `target/`. Don't — the dev build links to
Bevy dynamically. Always launch with `cargo run`.

**The build has printed nothing for five minutes**
That's expected during the first build. Leave it.

**You want the ~10 GB back**
From `afterlight/afterlight`, run `cargo clean`. The next `cargo run` will be slow
again, so only do this when you're finished.

---

## Quick reference

Everything above, once you're set up:

```bash
cd afterlight/afterlight
cargo run                # play
cargo check              # fast: does my change compile?
cargo build --release    # slow, optimised build (15+ min; only if you want the framerate)
```
