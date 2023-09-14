--!strict
-- test basic local scope
do
    local a = 1
    if true then
        local b = 2
        print(a)
    end
    print(b)
end
