print("========= BEGIN LUA TEST SECTION =========")

local straw = require("straw")
print("hostname ", straw.hostname)
print("username", straw.user)

-- print("args", straw.inspect(straw.args))

print("exists A", straw.fs.exists("~/.dotfiles"))
print("exists B", straw.fs.exists("~/.drotfiles"))
print("open A", straw.fs.open("~/.dotfiles"))
print("open B", straw.fs.open("~/.drotfiles"))
print("open C", straw.fs.open("~/.dotfiles/.straw/.gitignore"))
print("dotfiles path:", straw.fs.paths.dotfiles)
print("dotfiles path:", straw.fs.paths.home)

print("========= END LUA TEST SECTION =========")
