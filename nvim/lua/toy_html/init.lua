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

local function group(fg, bg)
  local name = "ToyHtml_" .. fg:sub(2) .. "_" .. bg:sub(2)
  if not groups[name] then
    vim.api.nvim_set_hl(0, name, { fg = fg, bg = bg })
    groups[name] = true
  end
  return name
end

local function draw(view, frame)
  local lines = {}
  for i, runs in ipairs(frame.lines) do
    local parts = {}
    for _, run in ipairs(runs) do
      parts[#parts + 1] = run[1]
    end
    lines[i] = table.concat(parts)
  end
  vim.bo[view.buf].modifiable = true
  vim.api.nvim_buf_set_lines(view.buf, 0, -1, false, lines)
  vim.bo[view.buf].modifiable = false
  vim.api.nvim_buf_clear_namespace(view.buf, ns, 0, -1)
  for i, runs in ipairs(frame.lines) do
    local byte = 0
    for _, run in ipairs(runs) do
      local len = #run[1]
      vim.api.nvim_buf_set_extmark(view.buf, ns, i - 1, byte, {
        end_col = byte + len,
        hl_group = group(run[2], run[3]),
      })
      byte = byte + len
    end
  end
end

local function handle(view, line)
  local ok, event = pcall(vim.json.decode, line)
  if not ok then
    return
  end
  if event.ev == "frame" then
    draw(view, event)
  elseif event.ev == "log" then
    local text = event.line:gsub("^%[log%] ", "")
    view.on_event(text)
  elseif event.ev == "error" then
    vim.notify("toy_html: " .. event.message, vim.log.levels.ERROR)
  end
end

local function send(view, command)
  vim.fn.chansend(view.job, vim.json.encode(command) .. "\n")
end

-- The cell under the cursor: row is the line, col counts display cells, not
-- bytes, which differ on any non-ASCII text.
local function cell(view)
  local row, byte = unpack(vim.api.nvim_win_get_cursor(view.win))
  local line = vim.api.nvim_buf_get_lines(view.buf, row - 1, row, false)[1] or ""
  return vim.fn.strdisplaywidth(line:sub(1, byte)), row - 1
end

local function click_cursor(view)
  local col, row = cell(view)
  send(view, { op = "click", col = col, row = row })
end

local function click_mouse(view)
  local pos = vim.fn.getmousepos()
  if pos.winid ~= view.win then
    return
  end
  local line = vim.api.nvim_buf_get_lines(view.buf, pos.line - 1, pos.line, false)[1] or ""
  local col = vim.fn.strdisplaywidth(line:sub(1, pos.column - 1))
  send(view, { op = "click", col = col, row = pos.line - 1 })
end

local function size(win)
  local info = vim.fn.getwininfo(win)[1]
  return info.width - info.textoff, info.height
end

-- Opens a view in a vertical split to the right of the current window, so an
-- ordinary file can stay open on the left.
--   opts.url   a page to load, or
--   opts.html  markup to show
function M.open(opts)
  opts = opts or {}
  vim.cmd("rightbelow vsplit")
  local win = vim.api.nvim_get_current_win()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_win_set_buf(win, buf)
  for name, value in pairs({
    number = false, relativenumber = false, wrap = false, list = false,
    signcolumn = "no", foldcolumn = "0", cursorline = false, spell = false,
  }) do
    vim.wo[win][name] = value
  end
  vim.bo[buf].bufhidden = "wipe"
  pcall(vim.api.nvim_buf_set_name, buf, "toy-html://" .. (opts.url or "markup"))

  local view = {
    buf = buf,
    win = win,
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
    if vim.api.nvim_win_is_valid(win) then
      local cols, rows = size(win)
      send(view, { op = "resize", cols = cols, rows = rows })
    end
  end
  vim.api.nvim_create_autocmd({ "WinResized", "VimResized" }, {
    callback = function()
      if vim.api.nvim_buf_is_valid(buf) then
        resize()
      end
    end,
  })

  local function scroll(rows)
    return function() send(view, { op = "scroll", rows = rows }) end
  end
  local function map(lhs, fn)
    vim.keymap.set("n", lhs, fn, { buffer = buf, nowait = true })
  end
  local _, rows = size(win)
  map("<CR>", function() click_cursor(view) end)
  map("<LeftMouse>", function() click_mouse(view) end)
  map("j", scroll(1))
  map("k", scroll(-1))
  map("<Down>", scroll(1))
  map("<Up>", scroll(-1))
  map("<ScrollWheelDown>", scroll(3))
  map("<ScrollWheelUp>", scroll(-3))
  map("<C-d>", scroll(math.floor(rows / 2)))
  map("<C-u>", scroll(-math.floor(rows / 2)))
  map("<Space>", scroll(rows - 1))
  map("<C-f>", scroll(rows - 1))
  map("<C-b>", scroll(-(rows - 1)))
  map("gg", scroll(-1000000))
  map("G", scroll(1000000))
  map("q", function() vim.api.nvim_win_close(win, true) end)

  resize()
  if opts.url then
    send(view, { op = "navigate", url = opts.url })
  else
    send(view, { op = "markup", html = opts.html or "" })
  end
  return view
end

return M
