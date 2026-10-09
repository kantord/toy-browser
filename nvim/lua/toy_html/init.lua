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

-- A row's text and runs, ready to be put in the buffer.
local function prepared(runs)
  local row, rest = trimmed(runs)
  local parts = {}
  for _, run in ipairs(row) do
    parts[#parts + 1] = run[1]
  end
  return table.concat(parts), row, rest
end

-- The highlights of a row that is already in the buffer: one over each run,
-- and one behind the whole line for the padding that was trimmed off.
local function mark_row(view, line, row, rest)
  vim.api.nvim_buf_clear_namespace(view.buf, ns, line, line + 1)
  if rest and rest[3] ~= "" then
    -- Low priority, so the runs' own colours win where there is text.
    vim.api.nvim_buf_set_extmark(view.buf, ns, line, 0, {
      line_hl_group = backdrop(rest[3]),
      priority = 1,
    })
  end
  local byte = 0
  for _, run in ipairs(row) do
    local len = #run[1]
    vim.api.nvim_buf_set_extmark(view.buf, ns, line, byte, {
      end_col = byte + len,
      hl_group = group(run[2], run[3], run[4]),
    })
    byte = byte + len
  end
end

-- A frame is the whole page, or (`at`) only the rows that changed since the last.
local function draw(view, frame)
  local first = frame.at
  local text, rows, rests = {}, {}, {}
  for n, runs in ipairs(frame.lines) do
    text[n], rows[n], rests[n] = prepared(runs)
  end
  vim.bo[view.buf].modifiable = true
  if first then
    for n, line in ipairs(first) do
      vim.api.nvim_buf_set_lines(view.buf, line, line + 1, false, { text[n] })
      mark_row(view, line, rows[n], rests[n])
    end
  else
    vim.api.nvim_buf_set_lines(view.buf, 0, -1, false, text)
    vim.api.nvim_buf_clear_namespace(view.buf, ns, 0, -1)
    for n = 1, #rows do
      mark_row(view, n - 1, rows[n], rests[n])
    end
  end
  vim.bo[view.buf].modifiable = false
end

-- Writes to the terminal itself, past Neovim's screen.
local function terminal(bytes)
  if vim.env.TMUX then
    -- tmux passes a sequence through only when it is wrapped, with its escapes doubled.
    bytes = "\27Ptmux;" .. bytes:gsub("\27", "\27\27") .. "\27\\"
  end
  if vim.api.nvim_ui_send then
    vim.api.nvim_ui_send(bytes)
  else
    vim.api.nvim_chan_send(vim.v.stderr, bytes)
  end
end

-- A picture to the terminal, by kitty's graphics protocol: transmitted once
-- under its number, and shown wherever the page's text has its placeholder
-- cells (the engine puts them in the buffer; this only has to deliver the image).
local function show_image(view, event)
  view.pictures[#view.pictures + 1] = event.id
  local chunk = 4096
  local data = event.png
  local first = true
  for at = 1, #data, chunk do
    local piece = data:sub(at, at + chunk - 1)
    local more = at + chunk <= #data and 1 or 0
    local control = first and ("a=T,U=1,f=100,q=2,i=%d,c=%d,r=%d,m=%d"):format(event.id, event.cols, event.rows, more)
      or ("m=%d"):format(more)
    terminal(("\27_G%s;%s\27\\"):format(control, piece))
    first = false
  end
end

local function forget_images(view)
  for _, id in ipairs(view.pictures) do
    terminal(("\27_Ga=d,d=I,q=2,i=%d\27\\"):format(id))
  end
  view.pictures = {}
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

-- The window showing this view, or nil: a page left behind in the jumplist is
-- a buffer no window is showing.
local function window(view)
  local win = vim.fn.bufwinid(view.buf)
  return win ~= -1 and win or nil
end

-- One host process serves every page: they share its fetch cache, and the
-- views are told apart by the number each of their commands and replies carries.
local handle
local host = { job = nil, views = {}, last = 0 }

local function start_job()
  local pending = ""
  host.job = vim.fn.jobstart({ M.host }, {
    on_stdout = function(_, data)
      -- jobstart splits on newlines; the first and last pieces may be partial.
      data[1] = pending .. data[1]
      pending = table.remove(data)
      for _, line in ipairs(data) do
        if line ~= "" then
          vim.schedule(function()
            local ok, event = pcall(vim.json.decode, line)
            local view = ok and host.views[event.page]
            if view and vim.api.nvim_buf_is_valid(view.buf) then
              handle(view, event)
            end
          end)
        end
      end
    end,
    on_exit = function()
      host.job = nil
      -- Pages the host held are gone with it; each is loaded again as it is shown.
      for _, view in pairs(host.views) do
        view.gone = true
      end
    end,
  })
end

local function send(view, command)
  if not view.page then
    return
  end
  if not host.job then
    start_job()
  end
  command.page = view.page
  vim.fn.chansend(host.job, vim.json.encode(command) .. "\n")
end


function handle(view, event)
  if event.ev == "frame" then
    status(view)
    draw(view, event)
    if view.cursor and window(view) then
      pcall(vim.api.nvim_win_set_cursor, window(view), view.cursor)
      view.cursor = nil
    end
  elseif event.ev == "log" then
    local text = event.line:gsub("^%[log%] ", "")
    view.on_event(text)
  elseif event.ev == "navigate" then
    local here = (view.url or ""):gsub("#.*", "")
    if event.url:gsub("#.*", "") ~= here and event.url:match("^[%w+.-]+:") then
      M.open({ url = event.url, here = true })
    else
      -- The same document: a #fragment, which is a place on the page.
      local name = event.url:match("#(.*)$")
      if name and name ~= "" then
        send(view, { op = "anchor", name = vim.uri_decode(name) })
      end
    end
  elseif event.ev == "focused" then
    local win = window(view)
    if win and event.row ~= vim.NIL and event.col ~= vim.NIL then
      pcall(vim.api.nvim_win_set_cursor, win, { event.row + 1, event.col })
    end
  elseif event.ev == "anchor" then
    local win = window(view)
    if win and event.row ~= vim.NIL then
      vim.api.nvim_win_call(win, function()
        vim.cmd("normal! m'")
      end)
      vim.api.nvim_win_set_cursor(win, { event.row + 1, 0 })
    end
  elseif event.ev == "pushed" then
    -- The page moved itself (history.pushState): that is a new entry in the
    -- history. The buffer it leaves is the snapshot of the page as it was; the
    -- running page carries on in a new one.
    M.open({ url = event.url, here = true, adopt = view })
  elseif event.ev == "image" then
    show_image(view, event)
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
--   opts.float show it in a floating window, centred: `true`, or {width=, height=};
--              the buffer is not listed and goes when the window does
--   opts.adopt a view whose running page this one takes over: the old buffer
--              stays as it was and is loaded from its own address if shown again
--   opts.images  "alt" (default): pictures as [their alt text]; "none": left out;
--              "real": shown, in a terminal that speaks kitty's graphics protocol
--              (kitty, ghostty, ...), as text the buffer can scroll and clip
--   opts.scheme  "light" (default) or "dark": what the page's prefers-color-scheme says
--   opts.transparent  leave the page's default white and black to the
--              colorscheme (default: only for `html`, not for a fetched `url`)
function M.open(opts)
  opts = opts or {}
  local buf = vim.api.nvim_create_buf(not opts.float, true)
  if opts.float then
    local width = math.min(opts.float.width or 60, vim.o.columns - 4)
    local height = math.min(opts.float.height or 20, vim.o.lines - 4)
    vim.api.nvim_open_win(buf, true, {
      relative = "editor",
      width = width,
      height = height,
      row = math.floor((vim.o.lines - height) / 2),
      col = math.floor((vim.o.columns - width) / 2),
      style = "minimal",
      border = "rounded",
    })
  else
    if opts.here then
      vim.cmd("normal! m'")
    else
      vim.cmd(opts.tab and "tabnew" or "rightbelow vsplit")
    end
    vim.api.nvim_win_set_buf(0, buf)
  end
  -- Kept when no window shows it: that is what makes it a page one can go
  -- back to, and its text is the snapshot of it.
  vim.bo[buf].bufhidden = opts.float and "wipe" or "hide"
  pcall(vim.api.nvim_buf_set_name, buf, "toy-html://" .. (opts.url or "markup"))

  local function newpage()
    host.last = host.last + 1
    return host.last
  end
  local view = {
    buf = buf,
    page = opts.adopt and opts.adopt.page or newpage(),
    url = opts.url,
    html = opts.html,
    -- A fetched page is drawn on white, as a browser would: it was designed for
    -- it, and sets its own text colours (usually dark) without setting a
    -- background, so a dark theme showing through would leave it unreadable.
    -- Markup a plugin supplies is its own, and takes the colorscheme.
    transparent = opts.transparent,
    images = opts.images or vim.g.toy_html_images,
    scheme = opts.scheme or vim.g.toy_html_scheme,
    pictures = {},
    on_event = opts.on_event or function(text)
      vim.notify(text)
    end,
  }
  if view.transparent == nil then
    view.transparent = opts.html ~= nil
  end
  host.views[view.page] = view
  if opts.adopt then
    opts.adopt.page, opts.adopt.closed = nil, true
    view.transparent, view.images = opts.adopt.transparent, opts.adopt.images
  end
  vim.api.nvim_create_autocmd("BufWipeout", {
    buffer = buf,
    once = true,
    callback = function()
      send(view, { op = "close" })
      forget_images(view)
      if view.page and host.views[view.page] == view then
        host.views[view.page] = nil
      end
    end,
  })

  -- `fresh`: this buffer has none of the page yet, so the frame must be whole.
  local function resize(silent, fresh)
    local win = window(view)
    if win then
      local cols, rows = size(win)
      send(view, { op = "resize", cols = cols, rows = rows, silent = silent, full = fresh })
    end
  end
  -- What the host needs to hold the page: sent once, and again if the host
  -- has forgotten the page. `quiet` keeps the buffer's text (the snapshot)
  -- while the page loads again behind it.
  local function load(quiet)
    if view.images == "real" then
      -- Pictures are drawn with true colour: it is how a cell names its picture.
      vim.o.termguicolors = true
    end
    if not view.page then
      view.page = newpage()
      host.views[view.page] = view
    end
    view.closed, view.gone = false, false
    send(view, { op = "transparent", on = view.transparent, silent = true })
    if view.images then
      send(view, { op = "images", mode = view.images, silent = true })
    end
    if view.scheme then
      send(view, { op = "scheme", mode = view.scheme, silent = true })
    end
    send(view, { op = "whole", on = true, silent = true })
    resize(true, true)
    if view.url then
      if quiet then
        -- The snapshot stays if the page cannot be had again.
        view.retried = true
      else
        say(view, "Loading " .. view.url .. " …")
      end
      send(view, { op = "navigate", url = view.url })
    else
      send(view, { op = "markup", html = view.html or "" })
    end
  end
  vim.api.nvim_create_autocmd("BufWinEnter", {
    buffer = buf,
    callback = function()
      set_window(vim.api.nvim_get_current_win())
      if view.closed or view.gone then
        status(view, "loading")
        load(true)
      else
        resize()
      end
    end,
  })
  -- A page no window shows is not kept in the host: its text stays in the
  -- buffer, and it is loaded again if it is shown again. Markup has no address
  -- to load it from, so it stays.
  vim.api.nvim_create_autocmd("BufWinLeave", {
    buffer = buf,
    callback = function()
      local win = vim.api.nvim_get_current_win()
      view.cursor = vim.api.nvim_win_get_cursor(win)
      restore_window(win)
      vim.schedule(function()
        if view.url and vim.api.nvim_buf_is_valid(buf) and not window(view) and not view.closed then
          view.closed = true
          send(view, { op = "close" })
          forget_images(view)
        end
      end)
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

  -- A click goes to the page first, so its own handlers run. If the page then
  -- lets the link be followed, the host reports a `navigate` instead of
  -- loading it (see `handle`): navigations are the jumplist's, so the link
  -- opens as a new page beside the old one in it and <C-o> returns.
  local function activate(row, byte)
    local col, line = cell_at(view, row, byte)
    send(view, { op = "click", col = col, row = line })
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
  -- Forms. Focus a field by clicking it (<CR>) or by tabbing to it, then type.
  map("]f", function() send(view, { op = "focus", dir = "next" }) end)
  map("[f", function() send(view, { op = "focus", dir = "prev" }) end)
  map("i", function()
    vim.ui.input({ prompt = "Type: " }, function(text)
      if text and text ~= "" then
        send(view, { op = "type", text = text })
      end
    end)
  end)
  -- Enter, in the field that has focus: submits a form, presses a button.
  map("gs", function() send(view, { op = "key", key = "Enter", code = "Enter" }) end)
  map("r", function()
    if view.url then
      view.retried = true
      status(view, "loading")
      send(view, { op = "navigate", url = view.url })
    end
  end)

  set_window(vim.api.nvim_get_current_win())
  if opts.adopt then
    status(view)
    resize(false, true)
  else
    status(view, opts.url and "loading")
    load(false)
  end
  return view
end

return M
