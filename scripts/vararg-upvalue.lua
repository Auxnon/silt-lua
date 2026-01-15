function counter1(...)
    local i=0
	return function()
        i=i+1 
        print(...,i)
    end
end

c=counter1()
c()
c()
c2=counter1()
c2()
c2()
c()

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
