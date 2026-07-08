-- function test(...)
--      local a,b=...
--     return a
-- end
-- return test(1,3,7,12)

function test()
	local a, b, c = 5, 6, 8
	b = 8
	return a + b + c
end
return test()
