-- :ToyHtml [url]   show a page in a vertical split (default: Wikipedia's Neovim article)
vim.api.nvim_create_user_command("ToyHtml", function(args)
  local url = args.args ~= "" and args.args or "https://en.wikipedia.org/wiki/Neovim"
  require("toy_html").open({ url = url })
end, { nargs = "?" })

-- :ToyHtmlGo {url}   load a page in this window; <C-o> comes back, like following a link
vim.api.nvim_create_user_command("ToyHtmlGo", function(args)
  require("toy_html").open({ url = args.args, here = true })
end, { nargs = 1 })

-- :ToyHtmlDemo   a small page that reports clicks through console.log
vim.api.nvim_create_user_command("ToyHtmlDemo", function()
  require("toy_html").open({
    html = [[
      <body style="margin:0;padding:8px;background:#1e1e2e;color:#cdd6f4">
        <h1 style="color:#f5c2e7">Pick a thing</h1>
        <ul>
          <li><a href="#" onclick="console.log('picked: apples')">apples</a></li>
          <li><a href="#" onclick="console.log('picked: bananas')">bananas</a></li>
        </ul>
        <button style="background:#89b4fa;color:#1e1e2e" onclick="console.log('picked: button')">Press me</button>
      </body>]],
  })
end, {})

-- :ToyHtmlHelp   the keys in a page, as a page in a float
vim.api.nvim_create_user_command("ToyHtmlHelp", function()
  require("toy_html").open({
    float = { width = 56, height = 16 },
    html = [[
      <body style="margin:0;padding:8px">
        <h1>Reading a page</h1>
        <ul>
          <li><b>&lt;CR&gt;</b> or click: follow a link</li>
          <li><b>r</b>: reload</li>
          <li><b>&lt;C-o&gt; / &lt;C-i&gt;</b>: back and forward</li>
          <li><b>q</b>: close this window</li>
        </ul>
        <p>Everything else is Neovim: search with <u>/</u>, yank with <u>y</u>.
        Pages follow your colorscheme.</p>
        <p><a href="https://github.com/neovim/neovim">Neovim on GitHub</a></p>
      </body>]],
  })
end, {})
