mt = debug.getmetatable(0) or {}
mt.__call = function(self, a)
    return self + a
end
debug.setmetatable(0, mt)

b = 0(3)

-- print(b) -- 5
