#!/usr/bin/env python3
"""Rank the game's own functions in a macOS `sample` call graph by inclusive time.

    tools/perf_sample_top.py game.sample.txt [top_n]

Each line of the call graph is "<indent><count> <function> ...". A function's
inclusive count is summed over the nodes where it is NOT already on the stack
above (so recursion isn't double counted), across every thread. Only
afterlight:: functions are listed (or another prefix, third argument), with Rust hashes and generics trimmed so
one system reads as one row.
"""
import re
import sys

NODE = re.compile(r"^([\s+!:|]*)(\d+)\s+(.+?)\s+\(in ")


def clean(fn):
    fn = re.sub(r"::h[0-9a-f]{16}$", "", fn)
    fn = re.sub(r"<[^<>]*>", "", fn)   # one level of generics; good enough
    fn = re.sub(r"<[^<>]*>", "", fn)
    return fn.strip()


def main(path, top=40):
    totals, stack, total_samples = {}, [], 0
    in_graph = False
    for line in open(path, errors="replace"):
        if line.startswith("Call graph:"):
            in_graph = True
            continue
        if in_graph and line.startswith("Total number in stack"):
            break
        if not in_graph:
            continue
        m = NODE.match(line)
        if not m:
            continue
        depth = len(m.group(1))
        count = int(m.group(2))
        fn = clean(m.group(3))
        while stack and stack[-1][0] >= depth:
            stack.pop()
        if not stack and "Thread_" in line:
            pass
        if depth <= 4:
            total_samples += count
        if fn.startswith(PREFIX) and all(f != fn for _, f in stack):
            totals[fn] = totals.get(fn, 0) + count
        stack.append((depth, fn))
    rows = sorted(totals.items(), key=lambda kv: -kv[1])[:top]
    print(f"== {path}: top {len(rows)} {PREFIX} functions by inclusive samples")
    for fn, c in rows:
        print(f"  {c:7d}  {fn}")


# Optional third argument: a function-name prefix other than afterlight::,
# e.g. "bevy_" to see what the engine itself spends.
PREFIX = sys.argv[3] if len(sys.argv) > 3 else "afterlight::"
main(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 40)
