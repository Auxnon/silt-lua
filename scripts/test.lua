do
    function outer()
        local y = 0
        local x = 2
        local function middle()
            local function inner()
                return x
            end
            y = y + 1
            return inner
        end

        y = y + 1
        x = x + y
        return middle
    end

    local a = outer()
    local b = a()
    return b()
end
