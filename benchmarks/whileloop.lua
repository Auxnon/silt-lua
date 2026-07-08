-- Tight while-loop benchmark with a branch inside.
local i = 0
local acc = 0
while i < 5000000 do
    i = i + 1
    if i % 2 == 0 then
        acc = acc + i
    end
end
print("acc = " .. acc)
return acc
