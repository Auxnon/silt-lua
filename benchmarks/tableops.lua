-- Table read/write benchmark: array fill then sum.
local t = {}
for i = 1, 1000000 do
    t[i] = i * 2
end
local sum = 0
for i = 1, 1000000 do
    sum = sum + t[i]
end
print("sum = " .. sum)
return sum
