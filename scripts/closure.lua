function create_counter()
    local count = 0
    local function f()
        count = count + 1
        print(count)
        return count
    end
    return f
end

create_counter()

local counter = create_counter()
counter()
counter()
