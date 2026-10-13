-- Resize benchmark, not part of the test suite (it needs the network):
--   TOY_BROWSER_HOST=<release host> nvim -u NONE --headless -l nvim/tests/bench_resize.lua
-- Per-stage times of a window resize on real pages, and how many frames a
-- burst of 20 resizes (a drag) makes the plugin apply.
local root = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h")
-- TOY_HTML_RTP: another copy of the plugin to compare against.
vim.opt.rtp:prepend(vim.env.TOY_HTML_RTP or (root .. "/nvim"))
local T = require("toy_html")
local now = vim.uv.hrtime
local function ms(ns) return ns / 1e6 end

-- Time spent inside the Neovim and JSON calls a frame is drawn with.
local spent, calls = {}, {}
local last_api = 0
local function timed(owner, name, label)
  local real = owner[name]
  owner[name] = function(...)
    local t = now()
    local a, b, c = real(...)
    spent[label] = (spent[label] or 0) + (now() - t)
    calls[label] = (calls[label] or 0) + 1
    last_api = now()
    return a, b, c
  end
end
timed(vim.json, "decode", "decode")
timed(vim.api, "nvim_buf_set_lines", "set_lines")
timed(vim.api, "nvim_buf_set_extmark", "extmarks")
timed(vim.api, "nvim_buf_clear_namespace", "clear_ns")
timed(vim.api, "nvim_set_hl", "set_hl")

-- Frames are told apart by the decode that yields one; the last send is when
-- the clock for the host's reply starts.
local sent_at, frames, last_frame = nil, {}, nil
local raw_decode = vim.json.decode
vim.json.decode = function(line, ...)
  local event = raw_decode(line, ...)
  if type(event) == "table" and event.ev == "frame" then
    frames[#frames + 1] = { arrived = now(), index = #frames + 1, latency = sent_at and now() - sent_at, bytes = #line }
  end
  return event
end
local real_chansend = vim.fn.chansend
vim.fn.chansend = function(job, data)
  if data:find('"op":"resize"', 1, true) then sent_at = now() end
  return real_chansend(job, data)
end
-- WinResized is only raised by a redraw, which a headless Neovim never does.
local function resize_to(w)
  vim.cmd("vertical resize " .. w)
  vim.api.nvim_exec_autocmds("WinResized", {})
end
local function reset()
  spent, calls, frames = {}, {}, {}
end

local function settle(quiet)
  local last = #frames
  while true do
    vim.wait(quiet)
    if #frames == last then return end
    last = #frames
  end
end

local function report(label, n, from)
  local line = { ("%-30s"):format(label) }
  local f = #frames
  line[#line + 1] = ("frames=%d"):format(f)
  if f > 0 then
    line[#line + 1] = ("host=%.1fms"):format(ms(frames[1].latency or 0))
    line[#line + 1] = ("%dKiB"):format(math.floor(frames[1].bytes / 1024))
    line[#line + 1] = ("apply(last frame)=%.1fms"):format(ms(last_api - frames[f].arrived))
  end
  for _, k in ipairs({ "decode", "set_lines", "extmarks", "clear_ns", "set_hl" }) do
    line[#line + 1] = ("%s=%.1fms(%d)"):format(k, ms(spent[k] or 0), calls[k] or 0)
  end
  print(table.concat(line, "  "))
end

local function bench(url)
  print("== " .. url)
  vim.cmd("silent! only")
  vim.cmd("enew")
  vim.cmd("vsplit")
  vim.cmd("wincmd h")
  local t0 = now()
  T.open({ url = url, here = true })
  vim.wait(60000, function() return #vim.api.nvim_buf_get_lines(0, 0, -1, false) > 5 end, 20)
  settle(300)
  print(("load %.0fms"):format(ms(now() - t0)))
  local width = vim.api.nvim_win_get_width(0)
  local win = vim.api.nvim_get_current_win()

  -- One resize.
  for _, w in ipairs({ width - 10, width, width - 10 }) do
    reset()
    local t = now()
    resize_to(w)
    vim.wait(2000, function() return #frames > 0 end, 1)
    local arrived = now()
    vim.wait(0)
    settle(60)
    report(("single resize to %d"):format(w), 1, t)
    print(("   resize->buffer updated: %.1fms"):format(ms(last_api - t)))
  end

  -- A drag: 20 resizes, one every 4ms.
  reset()
  local t = now()
  for i = 1, 20 do
    resize_to(width - 5 - (i % 10))
    vim.wait(4)
  end
  local last_resize = now()
  settle(800)
  local lastf = frames[#frames]
  report("drag of 20 resizes", 20, last_resize)
  print(("   last frame applied %.1fms after the last resize; drag took %.0fms"):format(
    lastf and ms(last_api - last_resize) or -1, ms(last_resize - t)))
  vim.api.nvim_win_set_width(win, width)
end

bench("https://en.wikipedia.org/wiki/Neovim")
bench("https://news.ycombinator.com")
vim.cmd("qa!")
