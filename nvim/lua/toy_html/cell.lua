-- How big one cell of the terminal is, in CSS pixels: what a page's width is
-- measured in (its `@media`, `vw` and `innerWidth` see columns * width). `nil`
-- when nothing is known, and the host then assumes 8 by 16.
--
-- Neovim cannot ask the terminal for it. `CSI 16 t` (cell size) and `CSI 14 t`
-- (window size) are answered with a CSI, and Neovim forwards only DA1, OSC, DCS
-- and APC replies to `TermResponse` (checked on 0.12), so a Lua plugin never
-- sees them. The operating system knows the same thing, though: the window size
-- of the terminal's pty has pixel fields (`ws_xpixel`, `ws_ypixel`), which
-- terminals, tmux and ssh fill in, and 0 means unknown.

local M = {}

-- `TIOCGWINSZ`, which is not the same number everywhere.
local function request()
  local system = vim.uv.os_uname().sysname
  return (system == "Darwin" or system:find("BSD")) and 0x40087468 or 0x5413
end

-- The pty's size as (columns, rows, width in px, height in px), or nothing.
-- Asked of whichever of the standard streams is still the terminal.
local function window()
  local ok, ffi = pcall(require, "ffi")
  if not ok then
    return
  end
  -- A second `cdef` of the same names raises, and they are declared then.
  pcall(ffi.cdef, [[
    struct toy_winsize { unsigned short rows, cols, xpixel, ypixel; };
    int ioctl(int fd, unsigned long request, ...);
  ]])
  local asked, size = pcall(function()
    local winsize = ffi.new("struct toy_winsize[1]")
    for _, fd in ipairs({ 1, 2, 0 }) do
      if ffi.C.ioctl(fd, request(), winsize) == 0 then
        return winsize[0]
      end
    end
  end)
  if asked and size then
    return size.cols, size.rows, size.xpixel, size.ypixel
  end
end

-- A cell from a window in pixels; nothing if the terminal does not say.
function M.of_window(cols, rows, xpixel, ypixel)
  if (cols or 0) > 0 and (rows or 0) > 0 and (xpixel or 0) > 0 and (ypixel or 0) > 0 then
    return xpixel / cols, ypixel / rows
  end
end

-- `{ width, height }` or `"WxH"`, as the user writes `vim.g.toy_html_cell_size`.
function M.parse(value)
  if type(value) == "string" then
    local width, height = value:match("^%s*([%d.]+)%s*[xX]%s*([%d.]+)%s*$")
    return tonumber(width), tonumber(height)
  elseif type(value) == "table" then
    return tonumber(value[1] or value.width), tonumber(value[2] or value.height)
  end
end

-- The user's own setting wins over anything measured.
function M.get()
  local width, height = M.parse(vim.g.toy_html_cell_size)
  if width and height then
    return width, height
  end
  return M.of_window(window())
end

return M
