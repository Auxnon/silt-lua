-- Numeric for-loop benchmark: tight arithmetic over many iterations.
local sum = 0
for i = 1, 10000000 do
    sum = sum + i
end
print("sum = " .. sum)
return sum
