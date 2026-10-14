-- Headless checks of the Neovim plugin against the real host:
--   nvim -u NONE --headless -l nvim/tests/run.lua      (just nvim-test)
-- Build the host first: cargo build -p toy-browser-host
local root = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h")
vim.opt.rtp:prepend(root .. "/nvim")
local T = require("toy_html")
local pages = root .. "/nvim/tests/pages/"
local failures = 0

local function check(name, ok, detail)
  io.stdout:write((ok and "ok   " or "FAIL ") .. name .. (ok and "" or ("  " .. vim.inspect(detail))) .. "\n")
  if not ok then failures = failures + 1 end
end
local function lines() return vim.api.nvim_buf_get_lines(0, 0, -1, false) end
local function has(text)
  for _, l in ipairs(lines()) do if l:find(text, 1, true) then return true end end
  return false
end
local function line_of(text)
  for i, l in ipairs(lines()) do if l:find(text, 1, true) then return i, l:find(text, 1, true) end end
end
local function wait_for(text) vim.wait(5000, function() return has(text) end, 50) end
local function press(keys) vim.api.nvim_feedkeys(vim.keycode(keys), "x", false); vim.wait(600) end
local events = {}
-- What a page console.log()s reaches the plugin as a notification.
vim.notify = function(message) events[#events + 1] = message end

-- A page, and a link followed from it into a new buffer.
vim.cmd("edit " .. root .. "/README.md")
local readme = vim.api.nvim_get_current_buf()
T.open({ url = "file://" .. pages .. "a.html", here = true })
wait_for("Page A")
local a = vim.api.nvim_get_current_buf()
check("page text is in the buffer", has("Page A"))
local row, col = line_of("to B")
vim.api.nvim_win_set_cursor(0, { row, col - 1 })
press("<CR>")
wait_for("Page B")
local b = vim.api.nvim_get_current_buf()
check("a link opens a new buffer", b ~= a and has("Page B"))

-- Back and forward restore pages; the one left is closed in the host and loaded again.
T.page("back"); wait_for("Page A")
check("back returns to the page", vim.api.nvim_get_current_buf() == a and has("Page A"))
T.page("forward"); wait_for("Page B")
check("forward returns to the next page", vim.api.nvim_get_current_buf() == b and has("Page B"))

-- Styling and wide characters.
local ns = vim.api.nvim_create_namespace("toy_html")
local styled = { bold = false, underline = false }
for _, m in ipairs(vim.api.nvim_buf_get_extmarks(0, ns, 0, -1, { details = true })) do
  local hl = m[4].hl_group and vim.api.nvim_get_hl(0, { name = m[4].hl_group }) or {}
  styled.bold = styled.bold or hl.bold
  styled.underline = styled.underline or hl.underline
end
check("bold and underline are highlights", styled.bold and styled.underline, styled)
check("wide characters keep their row", has("日本語 ok"))

-- A form: tab to a field, type into it, tab to the button, press it.
vim.ui.input = function(_, callback) callback("hello") end
press("]f"); press("i")
check("typing goes into the focused field", has("hello"), lines())
press("]f"); press("]f"); press("gs")
check("Enter on a button runs its handler", vim.tbl_contains(events, "sent"), events)

-- A #fragment link moves the cursor.
T.page("back"); wait_for("Page A")
row, col = line_of("jump")
vim.api.nvim_win_set_cursor(0, { row, col - 1 })
press("<CR>")
local at = line_of("the end")
check("a fragment link moves to its anchor", vim.fn.line(".") == at, { vim.fn.line("."), at })

-- The colour scheme: dark by default, switched for the open page at runtime.
local function colours_in_use()
  local names = {}
  for _, m in ipairs(vim.api.nvim_buf_get_extmarks(0, ns, 0, -1, { details = true })) do
    names[#names + 1] = m[4].hl_group or ""
  end
  return table.concat(names, " ")
end
local function wait_for_colour(hex) vim.wait(5000, function() return colours_in_use():find(hex, 1, true) end, 50) end
T.open({ url = "file://" .. pages .. "scheme.html", here = true })
wait_for("Scheme page"); wait_for_colour("101820")
check("pages are dark by default", colours_in_use():find("101820", 1, true), colours_in_use())
T.scheme("light"); wait_for_colour("ffffff")
check(":ToyHtmlScheme light re-renders the open page", colours_in_use():find("ffffff", 1, true), colours_in_use())

-- The size of a cell is the page's to be measured in: 80 columns are 640px
-- at the 8 by 16 nothing is known to be, and 800px at the 10 by 20 the user says.
T.open({ url = "file://" .. pages .. "cell.html", here = true })
wait_for("narrow")
check("a cell of unknown size is 8 by 16", has("narrow") and vim.o.columns == 80, lines())
vim.g.toy_html_cell_size = "10x20"
vim.api.nvim_exec_autocmds("VimResized", {})
wait_for("wide")
check("vim.g.toy_html_cell_size sets the width the page sees", has("wide"), lines())
vim.g.toy_html_cell_size = { 8, 16 }
vim.api.nvim_exec_autocmds("VimResized", {})
wait_for("narrow")
check("a table is taken as {width, height}, and the cell is sent again", has("narrow"), lines())
vim.g.toy_html_cell_size = nil
local measured = { require("toy_html.cell").of_window(100, 24, 960, 480) }
check("a window in pixels is a cell size", measured[1] == 9.6 and measured[2] == 20, measured)
check("a terminal that reports no pixels is no size", require("toy_html.cell").of_window(100, 24, 0, 0) == nil)

-- A drag raises a resize per cell: only the last size must reach the host,
-- and a click in the middle of it must not stop the page being redrawn.
T.open({ url = "file://" .. pages .. "a.html", here = true })
wait_for("Page A")
local sizes = {}
local real_chansend = vim.fn.chansend
vim.fn.chansend = function(job, data)
  local sent = vim.json.decode(data)
  if sent.op == "resize" then sizes[#sizes + 1] = sent.cols end
  return real_chansend(job, data)
end
vim.cmd("vsplit | wincmd h")
-- WinResized is only raised by a redraw, which a headless Neovim never does.
local function drag(width)
  vim.cmd("vertical resize " .. width)
  vim.api.nvim_exec_autocmds("WinResized", {})
end
sizes = {}
for width = 50, 31, -1 do drag(width) end
vim.wait(1000, function() return sizes[#sizes] == 31 end, 20)
check("the last size of a drag wins", sizes[#sizes] == 31, sizes)
check("a drag is not sent cell by cell", #sizes < 20, sizes)
sizes = {}
drag(45); drag(35)
press("<CR>")
drag(33)
vim.wait(1000, function() return sizes[#sizes] == 33 end, 20)
check("a resize after a click still goes out", sizes[#sizes] == 33, sizes)
check("the page is still drawn after them", has("Page A"))
vim.fn.chansend = real_chansend

io.stdout:write((failures == 0 and "all passed" or (failures .. " failed")) .. "\n")
vim.cmd(failures == 0 and "qa!" or "cq")
