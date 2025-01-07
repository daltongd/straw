print("========= BEGIN LUA TEST SECTION =========")

local straw = require("straw")
print("hostname ", straw.hostname)
print("username", straw.user)

-- print("args", straw.inspect(straw.args))

print("exists A", straw.fs.exists("~/.dotfiles"))
print("exists B", straw.fs.exists("~/.drotfiles"))

print("open C", "\n", straw.fs.read("~/.dotfiles/.straw/.gitignore"))

print("write bin: bytes written:", straw.fs.write_bin("~/.dotfiles/.straw/test.bin", { 250 }))
print("read bin:", "\n", straw.inspect(straw.fs.read_bin("~/.dotfiles/.straw/test.bin")))

print("filetype A", straw.fs.get_file_type("~/.dotfiles"))
print("filetype C", straw.fs.get_file_type("~/.dotfiles/.straw/.gitignore"))

print("dotfiles path:", straw.fs.paths.dotfiles)
print("dotfiles path:", straw.fs.paths.home)

straw.exec("ls", { "-la" })

print("ls output:", straw.exec("ls", { "-la" }, true))

print("========= END LUA TEST SECTION =========")
