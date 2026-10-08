-- Renders HTML in a Neovim buffer through toy-browser-host.
--
--   require("toy_html").open({
--     html = "<button onclick=\"console.log('hi')\">Hi</button>",
--     on_event = function(line) ... end,   -- whatever the page console.log()s
--   })
--
-- The host answers each command with a whole frame; it is drawn as buffer
-- lines plus one extmark per run of equal colour.

local M = {}

local ns = vim.api.nvim_create_namespace("toy_html")
local groups = {}

M.host = vim.env.TOY_BROWSER_HOST
  or vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h:h") .. "/target/debug/toy-browser-host"

-- An empty colour is the page's default one, left to the colorscheme: the
-- page is asked for it (`transparent`) so it takes on Neovim's own look.
local function group(fg, bg, flags)
  local name = "ToyHtml_" .. fg:sub(2) .. "_" .. bg:sub(2) .. "_" .. flags
  if not groups[name] then
    vim.api.nvim_set_hl(0, name, {
      fg = fg ~= "" and fg or nil,
      bg = bg ~= "" and bg or nil,
      bold = flags:find("b") ~= nil or nil,
      italic = flags:find("i") ~= nil or nil,
      underline = flags:find("u") ~= nil or nil,
    })
    groups[name] = true
  end
  return name
end

-- The runs of a row without the blank cells that only pad it out to the
-- window's width: they make `$` jump to the far edge and every yank carry
-- trailing blanks, so they are not put in the buffer. Their colour is not lost:
-- it is returned, to be put on the whole line instead. Blank cells have the
-- same colours as the page's text, so the padding is usually the tail of the
-- last run of text rather than a run of its own.
local function trimmed(runs)
  local rest = runs[#runs]
  if not rest then
    return runs, nil
  end
  local kept = #runs
  while kept > 0 and runs[kept][3] == rest[3] and not runs[kept][1]:find("%S") do
    kept = kept - 1
  end
  local out = {}
  for r = 1, kept do
    out[r] = runs[r]
  end
  local last = out[kept]
  if last and last[3] == rest[3] then
    out[kept] = { last[1]:gsub("%s+$", ""), last[2], last[3], last[4] }
  end
  return out, rest
end

-- A highlight of only a background, for the whole line: with a foreground too
-- it would paint every character of the row the same colour.
local function backdrop(bg)
  if bg == "" then
    return nil
  end
  local name = "ToyHtmlBackdrop_" .. bg:sub(2)
  if not groups[name] then
    vim.api.nvim_set_hl(0, name, { bg = bg })
    groups[name] = true
  end
  return name
end

local function draw(view, frame)
  local rows = {}
  local rests = {}
  local lines = {}
  for i, runs in ipairs(frame.lines) do
    rows[i], rests[i] = trimmed(runs)
    local parts = {}
    for _, run in ipairs(rows[i]) do
      parts[#parts + 1] = run[1]
    end
    lines[i] = table.concat(parts)
  end
  vim.bo[view.buf].modifiable = true
  vim.api.nvim_buf_set_lines(view.buf, 0, -1, false, lines)
  vim.bo[view.buf].modifiable = false
  vim.api.nvim_buf_clear_namespace(view.buf, ns, 0, -1)
  for i, runs in ipairs(rows) do
    if rests[i] and rests[i][3] ~= "" then
      -- Low priority, so the runs' own colours win where there is text.
      vim.api.nvim_buf_set_extmark(view.buf, ns, i - 1, 0, {
        line_hl_group = backdrop(rests[i][3]),
        priority = 1,
      })
    end
    local byte = 0
    for _, run in ipairs(runs) do
      local len = #run[1]
      vim.api.nvim_buf_set_extmark(view.buf, ns, i - 1, byte, {
        end_col = byte + len,
        hl_group = group(run[2], run[3], run[4]),
      })
      byte = byte + len
    end
  end
end

-- What the statusline of a page shows: its address, with what it is doing.
local function status(view, doing)
  vim.b[view.buf].toy_html_status = " " .. (doing and (doing .. " ") or "") .. (view.url or "markup")
end

-- Text standing in for the page until its first frame, or for good if it
-- never loads: the buffer is otherwise blank and says nothing.
local function say(view, text)
  vim.bo[view.buf].modifiable = true
  vim.api.nvim_buf_set_lines(view.buf, 0, -1, false, { text })
  vim.bo[view.buf].modifiable = false
end

local function handle(view, line)
  local ok, event = pcall(vim.json.decode, line)
  if not ok then
    return
  end
  if event.ev == "frame" then
    status(view)
    draw(view, event)
  elseif event.ev == "log" then
    local text = event.line:gsub("^%[log%] ", "")
    view.on_event(text)
  elseif event.ev == "target" then
    -- JSON null decodes to vim.NIL, which is truthy.
    view.target = { href = event.href ~= vim.NIL and event.href or nil }
  elseif event.ev == "error" then
    status(view, "failed:")
    if event.op ~= "navigate" or view.retried then
      vim.notify("toy_html: " .. event.message, vim.log.levels.ERROR)
    else
      say(view, "Could not load " .. (view.url or "the page") .. ": " .. event.message .. "  (r to retry)")
    end
  end
end

local function send(view, command)
  vim.fn.chansend(view.job, vim.json.encode(command) .. "\n")
end

-- The window showing this view, or nil: a page left behind in the jumplist is
-- a buffer no window is showing.
local function window(view)
  local win = vim.fn.bufwinid(view.buf)
  return win ~= -1 and win or nil
end

-- A position in the buffer as a cell: row is the line, col counts display
-- cells, not bytes, which differ on any non-ASCII text.
local function cell_at(view, row, byte)
  local line = vim.api.nvim_buf_get_lines(view.buf, row - 1, row, false)[1] or ""
  return vim.fn.strdisplaywidth(line:sub(1, byte)), row - 1
end

-- What is under a cell, asked of the engine and waited for (a few ms). The
-- answer is waited for rather than acted on from the reply because both callers
-- need it before they can finish: a menu that opens after the button has come
-- up is dismissed by the release that follows, and a click has to know whether
-- it is a link before deciding where it goes.
local function inspect(view, col, row)
  view.target = nil
  send(view, { op = "inspect", col = col, row = row })
  vim.wait(300, function() return view.target ~= nil end, 1)
  return view.target
end

local function size(win)
  local info = vim.fn.getwininfo(win)[1]
  return info.width - info.textoff, info.height
end

local ENTRY = [[PopUp.Open\ in\ new\ tab]]
local pending_href

-- Neovim's ordinary right-click menu, with "Open in new tab" added to it for
-- as long as the click was on a link. The engine only says what is under the
-- cell; that there are tabs, and a menu, is entirely this file's business.
--
-- The answer is waited for (a few ms) before the menu opens, rather than
-- opening it from the reply: a menu that appears after the button has already
-- come up is dismissed by the release that follows.
local NATIVE = [[PopUp.Open\ in\ web\ browser]]

-- `inside` is whether the click was in a page buffer. Neovim's own "Open in
-- web browser" opens the word under the cursor, which on a page is link text,
-- not the link: xdg-open "concept". So it is greyed out while a page has focus.
local function set_menu_entry(href, inside)
  pcall(vim.cmd, "silent aunmenu " .. ENTRY)
  pcall(vim.cmd, (inside and "amenu disable " or "amenu enable ") .. NATIVE)
  pending_href = href
  if href then
    vim.cmd("anoremenu 1 " .. ENTRY .. " <Cmd>lua require('toy_html').open_pending()<CR>")
  end
end

function M.open_pending()
  if pending_href then
    M.open({ url = pending_href, tab = true })
  end
end

-- Window options a page needs, saved and put back as a window starts and stops
-- showing one: they belong to the window, and <C-o> out of a page into a file
-- would otherwise leave that file unwrapped and without line numbers.
local WINDOW = {
  number = false, relativenumber = false, wrap = false, list = false,
  signcolumn = "no", foldcolumn = "0", cursorline = false, spell = false,
  statusline = "%{get(b:, 'toy_html_status', '')}%=%l/%L",
}

local function set_window(win)
  if vim.w[win].toy_html_saved then
    return
  end
  local saved = {}
  for name, value in pairs(WINDOW) do
    saved[name] = vim.wo[win][name]
    vim.wo[win][name] = value
  end
  vim.w[win].toy_html_saved = saved
end

local function restore_window(win)
  for name, value in pairs(vim.w[win].toy_html_saved or {}) do
    vim.wo[win][name] = value
  end
  vim.w[win].toy_html_saved = nil
end

-- The jumplist entry to land on to reach the nearest different buffer in a
-- direction, as the number of <C-o> (back) or <Tab> (forward) presses. This is
-- "previous page" as opposed to "previous jump": scrolling with G or searching
-- adds entries inside one page, and these skip them.
local function presses(direction)
  local list, at = unpack(vim.fn.getjumplist())
  local here = vim.api.nvim_get_current_buf()
  -- At the end of the list the position is not in it yet: the first <C-o>
  -- records it and then lands on the last entry.
  local index = at + 1
  local step = direction == "back" and -1 or 1
  local target = index + step
  if direction == "back" and at >= #list then
    target = #list
  end
  while list[target] and list[target].bufnr == here do
    target = target + step
  end
  if not list[target] then
    return nil
  end
  return direction == "back" and (at >= #list and (#list - target + 1) or (index - target))
    or (target - index)
end

function M.page(direction)
  local count = presses(direction)
  if count then
    vim.cmd("normal! " .. count .. (direction == "back" and "\15" or "\t"))
  end
end

-- Opens a page.
--   opts.url   a page to load, or
--   opts.html  markup to show
--   opts.here  show it in this window, replacing the page or file in it, with
--              the jump recorded so <C-o> comes back; otherwise a vertical
--              split to the right, so an ordinary file can stay open beside it
--   opts.tab   show it in a new tab page
--   opts.images  "alt" (default): pictures as [their alt text]; "none": left out
--   opts.transparent  leave the page's default white and black to the
--              colorscheme (default: only for `html`, not for a fetched `url`)
function M.open(opts)
  opts = opts or {}
  if opts.here then
    vim.cmd("normal! m'")
  else
    vim.cmd(opts.tab and "tabnew" or "rightbelow vsplit")
  end
  local buf = vim.api.nvim_create_buf(true, true)
  vim.api.nvim_win_set_buf(0, buf)
  -- Kept when no window shows it: that is what makes it a page one can go
  -- back to, and its text is the snapshot of it.
  vim.bo[buf].bufhidden = "hide"
  pcall(vim.api.nvim_buf_set_name, buf, "toy-html://" .. (opts.url or "markup"))

  local view = {
    buf = buf,
    url = opts.url,
    on_event = opts.on_event or function(text)
      vim.notify(text)
    end,
  }
  local pending = ""
  view.job = vim.fn.jobstart({ M.host }, {
    on_stdout = function(_, data)
      -- jobstart splits on newlines; the first and last pieces may be partial.
      data[1] = pending .. data[1]
      pending = table.remove(data)
      for _, line in ipairs(data) do
        if line ~= "" then
          vim.schedule(function()
            if vim.api.nvim_buf_is_valid(buf) then
              handle(view, line)
            end
          end)
        end
      end
    end,
  })
  vim.api.nvim_create_autocmd("BufWipeout", {
    buffer = buf,
    once = true,
    callback = function()
      vim.fn.jobstop(view.job)
    end,
  })

  local function resize()
    local win = window(view)
    if win then
      local cols, rows = size(win)
      send(view, { op = "resize", cols = cols, rows = rows })
    end
  end
  vim.api.nvim_create_autocmd("BufWinEnter", {
    buffer = buf,
    callback = function()
      set_window(vim.api.nvim_get_current_win())
      resize()
    end,
  })
  vim.api.nvim_create_autocmd("BufWinLeave", {
    buffer = buf,
    callback = function()
      restore_window(vim.api.nvim_get_current_win())
    end,
  })
  vim.api.nvim_create_autocmd({ "WinResized", "VimResized" }, {
    callback = function()
      if vim.api.nvim_buf_is_valid(buf) then
        resize()
      end
    end,
  })

  -- Everything else is Neovim's own: the page is real buffer text, so cursor
  -- motion, search, visual mode, yank and the mouse all work as usual. Only a
  -- click (a release in normal mode, so a drag is left to select) and <CR>
  -- reach the page. A right click asks the engine what is under it first.
  -- The menu is global; the entry must not follow the user into other buffers.
  vim.api.nvim_create_autocmd("BufLeave", {
    buffer = buf,
    callback = function()
      set_menu_entry(nil, false)
    end,
  })

  -- A click on a link is a navigation, and navigations are the jumplist's: the
  -- link opens as a new page beside the old one in it, so <C-o> returns. That
  -- means the page's own click handlers on a link are skipped; every other
  -- click still reaches the page.
  local function activate(row, byte)
    local col, line = cell_at(view, row, byte)
    local target = inspect(view, col, line)
    local href = target and target.href
    local here = (view.url or ""):gsub("#.*", "")
    if href and href:match("^[%w+.-]+:") and href:gsub("#.*", "") ~= here then
      M.open({ url = href, here = true })
    else
      send(view, { op = "click", col = col, row = line })
    end
  end
  local function map(lhs, fn)
    vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true })
  end
  local function activate_cursor()
    local row, byte = unpack(vim.api.nvim_win_get_cursor(0))
    activate(row, byte)
  end
  map("<CR>", activate_cursor)
  map("<LeftRelease>", function()
    if vim.fn.mode() == "n" then
      activate_cursor()
    end
  end)
  map("<A-Left>", function() M.page("back") end)
  map("<A-Right>", function() M.page("forward") end)
  map("<RightMouse>", function()
    local pos = vim.fn.getmousepos()
    local target
    if pos.winid == window(view) then
      local col, line = cell_at(view, pos.line, math.max(pos.column - 1, 0))
      target = inspect(view, col, line)
    end
    set_menu_entry(target and target.href, true)
    -- Then what Neovim would have done anyway ('mousemodel' popup_setpos):
    -- put the cursor there and open the menu.
    if pos.winid == window(view) then
      vim.api.nvim_win_set_cursor(pos.winid, { pos.line, math.max(pos.column - 1, 0) })
    end
    vim.cmd("popup PopUp")
  end)
  map("q", function() vim.api.nvim_win_close(0, true) end)
  map("r", function()
    if view.url then
      view.retried = true
      status(view, "loading")
      send(view, { op = "navigate", url = view.url })
    end
  end)

  -- A fetched page is drawn on white, as a browser would: it was designed for
  -- it, and sets its own text colours (usually dark) without setting a
  -- background, so a dark theme showing through would leave it unreadable.
  -- Markup a plugin supplies is its own, and takes the colorscheme.
  local transparent = opts.transparent
  if transparent == nil then
    transparent = opts.html ~= nil
  end
  status(view, opts.url and "loading")
  set_window(vim.api.nvim_get_current_win())
  send(view, { op = "transparent", on = transparent })
  local images = opts.images or vim.g.toy_html_images
  if images then
    send(view, { op = "images", mode = images })
  end
  send(view, { op = "whole", on = true })
  resize()
  if opts.url then
    say(view, "Loading " .. opts.url .. " …")
    send(view, { op = "navigate", url = opts.url })
  else
    send(view, { op = "markup", html = opts.html or "" })
  end
  return view
end

return M
