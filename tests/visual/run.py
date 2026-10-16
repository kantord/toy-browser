#!/usr/bin/env python3
"""Visual regression: made-up pages, each guarding a bug found on a real one.

For every tests/visual/pages/*.html:
  1. the host draws it (the engine's frame) and the grid is compared with
     tests/visual/expected/<page>.txt: the characters, then which colour is
     behind and in front of each, as letters named in a legend;
  2. if tmux and nvim exist, the Neovim plugin shows the same page in a real
     terminal and what the terminal paints must equal the host's frame: a
     colour lost between the engine and the screen is a bug of its own.

  python3 tests/visual/run.py            check
  python3 tests/visual/run.py --bless    rewrite the expected files (read the diff)
"""
import json, pathlib, re, shutil, subprocess, sys, time

ROOT = pathlib.Path(__file__).resolve().parents[2]
HOST = ROOT / "target/release/toy-browser-host"
PAGES = sorted((ROOT / "tests/visual/pages").glob("*.html"))
EXPECTED = ROOT / "tests/visual/expected"
COLS, ROWS = 80, 24


def host_frame(page):
    """The host's whole frame for a page: rows of (char, fg, bg) cells."""
    p = subprocess.Popen([str(HOST)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    ops = [
        {"op": "transparent", "on": False, "silent": True},
        {"op": "scheme", "mode": "dark", "silent": True},
        {"op": "whole", "on": True, "silent": True},
        {"op": "cell", "width": 8, "height": 16, "silent": True},
        {"op": "resize", "cols": COLS, "rows": ROWS, "silent": True},
        {"op": "navigate", "url": page.as_uri(), "silent": True},
        {"op": "resize", "cols": COLS, "rows": ROWS, "full": True},
    ]
    for op in ops:
        p.stdin.write(json.dumps(op) + "\n")
    p.stdin.flush()
    frame = None
    for line in p.stdout:
        event = json.loads(line)
        if event.get("ev") == "frame":
            frame = event
            break
    p.kill()
    return [[(text[i], fg, bg) for text, fg, bg, _ in row for i in range(len(text))] for row in frame["lines"]]


def golden(grid):
    """Characters, then two colour maps whose letters are named in a legend."""
    names = {}

    def letter(color):
        return names.setdefault(color, chr(ord("a") + len(names)))

    chars = ["".join(c for c, _, _ in row).rstrip() for row in grid]
    back = ["".join(letter(bg) for _, _, bg in row) for row in grid]
    front = ["".join(letter(fg) if c != " " else "." for c, fg, _ in row) for row in grid]
    legend = "  ".join(f"{k}={v}" for v, k in names.items())
    last = max((i for i, t in enumerate(chars) if t), default=0) + 1
    return "\n".join(["legend  " + legend, "text", *chars[:last], "behind", *back[:last], "in front", *front[:last], ""])


def screen(page):
    """What a real terminal paints for the plugin showing the page: (char, fg, bg) per cell."""
    name = "toyvisual"
    subprocess.run(["tmux", "kill-session", "-t", name], capture_output=True)
    cmd = (f"TOY_BROWSER_HOST={HOST} nvim -u NONE --cmd 'set rtp+={ROOT}/nvim' "
           f"--cmd 'set termguicolors showtabline=0 laststatus=0 noruler noshowcmd' "
           f"--cmd 'let g:toy_html_cell_size=\"8x16\"' "
           f"-c 'runtime plugin/toy_html.lua' -c 'ToyHtmlGo {page.as_uri()}'")
    subprocess.run(["tmux", "new-session", "-d", "-s", name, "-x", str(COLS), "-y", str(ROWS), cmd], check=True)
    try:
        before, stable = None, 0
        for _ in range(100):
            time.sleep(0.2)
            now = subprocess.run(["tmux", "capture-pane", "-e", "-N", "-p", "-t", name], capture_output=True, text=True).stdout
            stable = stable + 1 if now == before and "Loading" not in now else 0
            before = now
            if stable >= 4:
                break
        return parse_ansi(before)
    finally:
        subprocess.run(["tmux", "kill-session", "-t", name], capture_output=True)


def parse_ansi(text):
    rows, fg, bg = [], None, None  # colours carry over from one line to the next
    for line in text.split("\n")[:ROWS]:
        row, i = [], 0
        while i < len(line):
            m = re.match(r"\x1b\[([0-9;]*)m", line[i:])
            if m:
                codes = [int(x or 0) for x in m.group(1).split(";")]
                j = 0
                while j < len(codes):
                    if codes[j] == 0:
                        fg = bg = None
                    elif codes[j] in (38, 48) and codes[j + 1:j + 2] == [2]:
                        color = "#%02x%02x%02x" % tuple(codes[j + 2:j + 5])
                        fg, bg = (color, bg) if codes[j] == 38 else (fg, color)
                        j += 4
                    j += 1
                i += m.end()
            else:
                row.append((line[i], fg, bg))
                i += 1
        rows.append(row)
    return rows


def disagreements(frame, shown):
    """Cells where the terminal's background is not the host's (text rows only)."""
    out = []
    for y, row in enumerate(frame[: min(len(frame), ROWS)]):
        if not "".join(c for c, _, _ in row).strip():
            continue
        for x, (c, fg, bg) in enumerate(row[:COLS]):
            cell = shown[y][x] if y < len(shown) and x < len(shown[y]) else (" ", None, None)
            if cell[2] != bg or (c != " " and cell[0] != c):
                out.append(f"row {y} col {x}: host {c!r} on {bg}, terminal {cell[0]!r} on {cell[2]}")
                break
    return out


def main():
    bless = "--bless" in sys.argv
    if not HOST.exists():
        sys.exit(f"build the host first: cargo build --release -p toy-browser-host ({HOST})")
    terminal = shutil.which("tmux") and shutil.which("nvim")
    EXPECTED.mkdir(exist_ok=True)
    failed = 0
    for page in PAGES:
        frame = host_frame(page)
        text = golden(frame)
        want = EXPECTED / (page.stem + ".txt")
        problems = []
        if bless:
            want.write_text(text)
        elif not want.exists() or want.read_text() != text:
            problems.append(f"the frame changed: diff <(python3 tests/visual/run.py --show {page.stem}) {want.relative_to(ROOT)}")
        if terminal:
            problems += disagreements(frame, screen(page))
        print(("FAIL " if problems else "ok   ") + page.name)
        for problem in problems[:5]:
            print("     " + problem)
        failed += bool(problems)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    if "--show" in sys.argv:
        print(golden(host_frame(next(p for p in PAGES if p.stem == sys.argv[-1]))), end="")
    else:
        main()
