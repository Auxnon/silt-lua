-- Nested closure / recursion benchmark (naive recursive Fibonacci).
-- Exercises call frames, returns, and arithmetic. Comparable to PUC-Lua's classic fib bench.
local function fib(n)
    if n < 2 then return n end
    return fib(n - 1) + fib(n - 2)
end

local N = 30
local result = fib(N)
print("fib(" .. N .. ") = " .. result)
return result
