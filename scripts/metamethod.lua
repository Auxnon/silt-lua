local meta ={}
meta.__add=function(a,b)
    print('yeeeeh')
    return a..b
end

debug.setmetatable(0,meta)

print(2+'3')
