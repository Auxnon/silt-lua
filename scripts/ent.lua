function fun(g)
	local a = test_ent()
	print("we made ent")
	print("it has " .. a.x)
	print("it has " .. g)
	-- a.x=10
	-- print("it has "..a.x)
	local b = { x = 8, y = 7, z = 6 }
	a:pos(b)
	return a.x
end
b = fun(6)
--
-- b={x=9,4,y=7,z=6}
-- -- b={}
-- -- b.x=9
-- b[1]
