-- Nested-closure benchmark: many closures each capturing an upvalue, then invoked in a loop.
-- (Mirrors the per-entity update-closure pattern common in game scripting.)
local function make_adder(n)
    return function(x) return x + n end
end

local total = 0
for i = 1, 1000000 do
    local add = make_adder(i)
    total = add(total)
end
print("total = " .. total)
return total
