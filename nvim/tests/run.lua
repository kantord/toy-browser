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

io.stdout:write((failures == 0 and "all passed" or (failures .. " failed")) .. "\n")
vim.cmd(failures == 0 and "qa!" or "cq")
