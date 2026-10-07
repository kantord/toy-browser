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

    def to_html(self, refresh=False):
        out = []
        for r, row in enumerate(self.cells):
            runs, last = [], None
            for c, (text, hl) in enumerate(row):
                key = (hl, self.cursor == (r, c))
                if key != last:
                    runs.append([key, ""])
                    last = key
                runs[-1][1] += text if text else " "
            spans = []
            for (hl, cur), text in runs:
                fg, bg = self.colours(hl, cur)
                spans.append(
                    f'<span style="color:{fg};background:{bg}">{htmllib.escape(text)}</span>'
                )
            out.append("".join(spans))
        meta = '<meta http-equiv="refresh" content="1">' if refresh else ""
        return (
            f'<!doctype html><meta charset="utf-8">{meta}'
            f'<body style="margin:0;background:#{self.bg:06x}"><pre style="margin:0">'
            + "\n".join(out)
            + "</pre></body>"
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


def paint(frame):
    """A frame from the host, as ANSI on the real terminal."""
    out = ["\x1b[H"]
    for runs in frame["lines"]:
        for text, fg, bg in runs:
            f = [int(fg[i : i + 2], 16) for i in (1, 3, 5)]
            b = [int(bg[i : i + 2], 16) for i in (1, 3, 5)]
            out.append(f"\x1b[38;2;{f[0]};{f[1]};{f[2]};48;2;{b[0]};{b[1]};{b[2]}m{text}")
        out.append("\x1b[0m\r\n")
    sys.stdout.write("".join(out))
    sys.stdout.flush()


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
    next_frame(host)  # every command is answered with a frame

    grid = Grid()
    old = termios.tcgetattr(0)
    tty.setraw(0)
    sys.stdout.write("\x1b[?1049h\x1b[2J")
    try:
        while nvim.poll() is None:
            ready, _, _ = select.select([0, nvim.stdout], [], [])
            if 0 in ready:
                data = os.read(0, 1024)
                nvim.stdin.write(msgpack.packb([2, "nvim_input", [data.decode(errors="replace")]]))
                nvim.stdin.flush()
            if nvim.stdout in ready:
                unpacker.feed(os.read(nvim.stdout.fileno(), 65536))
                flushed = False
                for msg in unpacker:
                    if msg[0] == 2 and msg[1] == "redraw":
                        apply(grid, msg[2])
                        flushed |= any(e[0] == "flush" for e in msg[2])
                if flushed:
                    document = grid.to_html()
                    with open(path + ".tmp", "w") as f:
                        f.write(grid.to_html(refresh=True))
                    os.replace(path + ".tmp", path)
                    host_send(host, {"op": "markup", "html": document})
                    paint(next_frame(host))
    finally:
        termios.tcsetattr(0, termios.TCSADRAIN, old)
        sys.stdout.write("\x1b[?1049l")
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
