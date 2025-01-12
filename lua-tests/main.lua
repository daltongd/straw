print("========= BEGIN LUA TEST SECTION =========")

local straw = require("straw")
print("hostname ", straw.hostname)
print("username", straw.user)

-- print("args", straw.inspect(straw.args))

print("exists A", straw.fs.exists("~/.dotfiles"))
print("exists B", straw.fs.exists("~/.drotfiles"))

print("open C", "\n", straw.fs.read("~/.dotfiles/.straw/.gitignore"))

straw.fs.create_dir("~/.dotfiles/.straw/.tmp")
print("write bin: bytes written:", straw.fs.write_bin("~/.dotfiles/.straw/.tmp/test.bin", { 255, 0, 255 }))
print("read bin:", "\n", straw.inspect(straw.fs.read_bin("~/.dotfiles/.straw/.tmp/test.bin")))

print("write txt: bytes written:", straw.fs.write("~/.dotfiles/.straw/.tmp/test.txt", "yo"))
print("write txt empty: bytes written:", straw.fs.write("~/.dotfiles/.straw/.tmp/empty.txt"))

local paths =
	{ "~/.dotfiles/.straw/.tmp/test.bin", "~/.dotfiles/.straw/.tmp/test.txt", "~/.dotfiles/.straw/.tmp/empty.txt" }
for _, path in ipairs(paths) do
	if straw.fs.exists(path) then
		print("deleting file:", path)
		straw.fs.remove(path)
	end
end
straw.fs.remove_dir("~/.dotfiles/.straw/.tmp")

print("filetype A", straw.fs.get_file_type("~/.dotfiles"))
print("filetype C", straw.fs.get_file_type("~/.dotfiles/.straw/.gitignore"))

print("dotfiles path:", straw.fs.paths.dotfiles)
print("dotfiles path:", straw.fs.paths.home)

straw.exec("ls", { "-la" })

print("ls output:", straw.exec("ls", { "-la" }, true))

print("========= END LUA TEST SECTION =========")
