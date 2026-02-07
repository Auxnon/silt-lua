
function test()
    local t={}
    -- t.g=7
    t[1]=2
    t[3]=4

    t[2]=4
    -- t[1]=nil
    table.insert(t,"2a",6)
    -- table.remove(t,1)
    -- for i = 1, 3, 1 do
    --    print("i" ..i..":".. (t[i] or "_") )
    -- end
    print("size"..#t)
end
test()
