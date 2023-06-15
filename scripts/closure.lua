
function create_counter()
    local count = 0
    return function()
        count = count + 1
        print(count)
        return count
    end
end

local counter = create_counter()
counter()
counter()