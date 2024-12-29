print("========= BEGIN LUA TEST SECTION =========")

local straw = require("straw")
print("hostname ", straw.hostname)
print("username", straw.user)

print("args", straw.inspect(straw.args))

print("========= END LUA TEST SECTION =========")
