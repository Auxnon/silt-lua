function test(a, ...)
    b=...
	print( b)
end

print(nil)
test(2,2,3)

-- a, b, c = test(2, 5, 6)
-- print(a, b, c)
--
-- function test2(...)
-- 	return { ... }
-- end
--
-- t = test2(...)
-- print('size'..#t)
--
-- for key, value in pairs(t) do
-- 	print(key, "=", value)
-- end
