#!/usr/bin/env python3
"""A Neovim UI whose screen is HTML.

Attaches to `nvim --embed` over msgpack-rpc with ext_linegrid, keeps the grid,
and on every `flush` turns it into one HTML document. That document goes to:

  * toy-browser-host, which draws it as cells in this terminal, and
  * a file (default /tmp/nvim.html) a real web browser can open — it reloads
    itself once a second.

Keys typed here go to Neovim. Usage: nvim_html.py [--html FILE] [nvim args...]
"""
import html as htmllib
import json
import os
import re
import select
import subprocess
import sys
import termios
import tty

import msgpack

HOST = os.environ.get(
    "TOY_BROWSER_HOST",
    os.path.join(os.path.dirname(__file__), "..", "..", "target", "debug", "toy-browser-host"),
)


class Grid:
    def __init__(self):
        self.cols = self.rows = 0
        self.cells = []  # rows of [text, hl_id]
        self.hl = {0: {}}
        self.fg, self.bg = 0xD4D4D4, 0x1E1E1E
        self.cursor = (0, 0)

    def resize(self, cols, rows):
        self.cols, self.rows = cols, rows
        self.cells = [[[" ", 0] for _ in range(cols)] for _ in range(rows)]

    def line(self, row, col, cells):
        hl = 0
        for cell in cells:
            text = cell[0]
            if len(cell) > 1:
                hl = cell[1]
            for _ in range(cell[2] if len(cell) > 2 else 1):
                if col < self.cols:
                    self.cells[row][col] = [text, hl]
                col += 1

    def scroll(self, top, bot, left, right, rows):
        src = range(top + rows, bot) if rows > 0 else range(bot - 1 + rows, top - 1, -1)
        for r in src:
            if 0 <= r - rows < self.rows:
                self.cells[r - rows][left:right] = [list(c) for c in self.cells[r][left:right]]

    def colours(self, hl_id, cursor=False):
        attrs = self.hl.get(hl_id, {})
        fg = attrs.get("foreground", self.fg)
        bg = attrs.get("background", self.bg)
        if attrs.get("reverse") != cursor:
            fg, bg = bg, fg
        return f"#{fg:06x}", f"#{bg:06x}"

    def row_html(self, r):
        """The spans of one row: runs of cells with the same highlight and cursor."""
        runs, last = [], None
        for c, (text, hl) in enumerate(self.cells[r]):
            key = (hl, self.cursor == (r, c))
            if key != last:
                runs.append([key, ""])
                last = key
            runs[-1][1] += text if text else " "
        spans = []
        for (hl, cur), text in runs:
            fg, bg = self.colours(hl, cur)
            spans.append(f'<span style="color:{fg};background:{bg}">{htmllib.escape(text)}</span>')
        return "".join(spans)

    def rows_html(self):
        return [self.row_html(r) for r in range(self.rows)]

    def to_html(self, rows, refresh=False):
        # One element per row, with a fixed id and key: the structure is the same
        # every frame, only what is inside a row changes, and the host can be
        # patched one row at a time (`data-key`).
        body = "".join(
            f'<div class="row" id="r{r}" data-key="r{r}">{row}</div>' for r, row in enumerate(rows)
        )
        meta = '<meta http-equiv="refresh" content="1">' if refresh else ""
        return (
            f'<!doctype html><meta charset="utf-8">{meta}'
            f'<body style="margin:0;background:#{self.bg:06x}">'
            '<div id="screen" style="margin:0;white-space:pre;font-family:monospace">'
            + body
            + "</div></body>"
        )


def apply(grid, events):
    for name, *calls in events:
        for a in calls:
            if name == "grid_resize":
                grid.resize(a[1], a[2])
            elif name == "default_colors_set":
                grid.fg, grid.bg = a[0], a[1]
            elif name == "hl_attr_define":
                grid.hl[a[0]] = a[1]
            elif name == "grid_line":
                grid.line(a[1], a[2], a[3])
            elif name == "grid_clear":
                grid.resize(grid.cols, grid.rows)
            elif name == "grid_cursor_goto":
                grid.cursor = (a[1], a[2])
            elif name == "grid_scroll":
                grid.scroll(a[1], a[2], a[3], a[4], a[5])


class Screen:
    """What the host last drew, so a frame that carries only changed rows can
    be applied to it."""

    def __init__(self):
        self.lines = []

    def apply(self, frame):
        if "at" in frame:
            for row, runs in zip(frame["at"], frame["lines"]):
                self.lines[row] = runs
        else:
            self.lines = frame["lines"]


def paint(screen, frame):
    """A frame from the host, as ANSI on the real terminal."""
    screen.apply(frame)
    out = ["\x1b[H"]
    for runs in screen.lines:
        for text, fg, bg, _flags in runs:
            f = [int(fg[i : i + 2], 16) for i in (1, 3, 5)]
            b = [int(bg[i : i + 2], 16) for i in (1, 3, 5)]
            out.append(f"\x1b[38;2;{f[0]};{f[1]};{f[2]};48;2;{b[0]};{b[1]};{b[2]}m{text}")
        out.append("\x1b[0m\r\n")
    sys.stdout.write("".join(out))
    sys.stdout.flush()


# SGR mouse reports from the terminal: ESC [ < button ; column ; row (M press | m release)
MOUSE = re.compile(r"\x1b\[<(\d+);(\d+);(\d+)([Mm])")
BUTTONS = {0: "left", 1: "middle", 2: "right"}


def mouse_calls(data):
    """The keys in `data` and the mouse reports in it, split apart: Neovim takes
    them through different calls."""
    calls, rest, last = [], [], 0
    for m in MOUSE.finditer(data):
        rest.append(data[last : m.start()])
        last = m.end()
        code, col, row, kind = int(m[1]), int(m[2]) - 1, int(m[3]) - 1, m[4]
        if code & 64:
            calls.append(("wheel", "up" if code & 1 == 0 else "down", "press", row, col))
        elif code & 32:
            calls.append((BUTTONS.get(code & 3, "left"), "drag", "", row, col))
        else:
            calls.append((BUTTONS.get(code & 3, "left"), "press" if kind == "M" else "release", "", row, col))
    rest.append(data[last:])
    return "".join(rest), calls


def main():
    args = sys.argv[1:]
    path = "/tmp/nvim.html"
    if args[:1] == ["--html"]:
        path, args = args[1], args[2:]
    cols, rows = os.get_terminal_size()
    rows -= 1

    nvim = subprocess.Popen(
        ["nvim", "--embed", *args], stdin=subprocess.PIPE, stdout=subprocess.PIPE
    )
    host = subprocess.Popen(
        [HOST], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
    )
    unpacker = msgpack.Unpacker(raw=False)
    nvim.stdin.write(
        msgpack.packb([0, 1, "nvim_ui_attach", [cols, rows, {"ext_linegrid": True, "rgb": True}]])
    )
    nvim.stdin.flush()
    host_send(host, {"op": "resize", "cols": cols, "rows": rows})
    screen = Screen()
    screen.apply(next_frame(host))  # every command is answered with a frame

    grid = Grid()
    shown = None  # the rows the host has, as HTML
    old = termios.tcgetattr(0)
    tty.setraw(0)
    sys.stdout.write("\x1b[?1049h\x1b[2J\x1b[?1000h\x1b[?1002h\x1b[?1006h")
    try:
        while nvim.poll() is None:
            ready, _, _ = select.select([0, nvim.stdout], [], [])
            if 0 in ready:
                data, calls = mouse_calls(os.read(0, 1024).decode(errors="replace"))
                if data:
                    nvim.stdin.write(msgpack.packb([2, "nvim_input", [data]]))
                for button, action, modifier, row, col in calls:
                    nvim.stdin.write(
                        msgpack.packb([2, "nvim_input_mouse", [button, action, modifier, 0, row, col]])
                    )
                nvim.stdin.flush()
            if nvim.stdout in ready:
                unpacker.feed(os.read(nvim.stdout.fileno(), 65536))
                flushed = False
                for msg in unpacker:
                    if msg[0] == 2 and msg[1] == "redraw":
                        apply(grid, msg[2])
                        flushed |= any(e[0] == "flush" for e in msg[2])
                if flushed:
                    rows = grid.rows_html()
                    with open(path + ".tmp", "w") as f:
                        f.write(grid.to_html(rows, refresh=True))
                    os.replace(path + ".tmp", path)
                    if shown is None or len(shown) != len(rows):
                        host_send(host, {"op": "markup", "html": grid.to_html(rows)})
                    else:
                        changed = [r for r, row in enumerate(rows) if row != shown[r]]
                        if not changed:
                            continue
                        for r in changed:
                            # Only the last is answered with a frame.
                            host_send(host, {"op": "patch", "key": f"r{r}", "html": rows[r],
                                             "silent": r != changed[-1]})
                    shown = rows
                    paint(screen, next_frame(host))
    finally:
        termios.tcsetattr(0, termios.TCSADRAIN, old)
        sys.stdout.write("\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[?1049l")
        host.kill()


def host_send(host, command):
    host.stdin.write(json.dumps(command) + "\n")
    host.stdin.flush()


def next_frame(host):
    while True:
        event = json.loads(host.stdout.readline())
        if event["ev"] == "frame":
            return event


if __name__ == "__main__":
    main()
