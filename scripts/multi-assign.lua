
local x=4
local y=5
-- z,w=x+y,x-y
function test()
    return x+x+y,y+y+y
end
-- z,w=x+y+6,y+2+y
z,w=test()
return w
-- assert(x==9,"multi assign failed for x")
-- assert(y==-1,"multi assign failed for x")
