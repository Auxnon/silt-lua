-- String concatenation benchmark (allocation-heavy).
local s = ""
for i = 1, 50000 do
    s = s .. "x"
end
print("len = " .. #s)
return #s
