-- test local scope index is correctly evaluated in compiler
do
    function test(bool)
        local x = 1
        local b = 4
        if bool then
            local d = 5
            local e = 6
            local f = 7
            x = x + e
        end
        x = x + b
        return x
    end

    return test(true) + test(false) -- 11 + 5 = 16
end
