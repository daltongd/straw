local straw = require("straw")

local function remove(path)
	if straw.fs.exists(path) then
		print("removing " .. path)
		straw.fs.remove_dir(path, true)
	end
end

local function clone(url, path)
	remove(path)
	print("cloning " .. url .. " to " .. path)
	straw.git.clone(url, path)
end

local function init(path, url)
	straw.fs.create_dir(path, true)
	print("git init " .. path .. " with origin " .. url)
	straw.git.init(path, url)
end

local function commit(path, message)
	print("git commit '" .. message .. "' path: " .. path)
	straw.git.commit(path, message)
end

local function status(path)
	print("getting status")
	local statuses = straw.git.status(path)
	for _, status_entry in ipairs(statuses) do
		print("status: " .. status_entry)
	end
end

local url = "https://github.com/daltongd/UnrealSuite.git"
local path = straw.fs.paths.home .. "/.straw/.tmp/UnrealSuite"

clone(url, path)
remove(path)

path = straw.fs.paths.home .. "/.straw/.tmp/test"
remove(straw.fs.paths.home .. "/.straw")
init(path, url)
local git_path = path .. "/.git"
if straw.fs.exists(git_path) then
	print("directory exists: " .. git_path)
end

commit(path, "repo init")

local file_path = path .. "/.keeper"
print("creating " .. file_path)
local bytes_written = straw.fs.write(file_path)
print("written " .. bytes_written .. " bytes")

status(path)
commit(path, "feat: add .keeper")
status(path)

remove(straw.fs.paths.home .. "/.straw")
