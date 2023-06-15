
count2=0
function create_counter()
    local count = 0
    return function()
        count = count + 1
        count2 = count2 + 1
        print(count..":"..count2)
        return count
    end
end


function caller()
    local counter = create_counter()
    local count2=10
    counter()
    counter()
end

local counter = create_counter()
counter()
counter()