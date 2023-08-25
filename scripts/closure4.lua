function outer()
    local x = "value"
    local function middle()
        local function inner()
            print(x)
        end
        print("created inner closure")
        return inner
    end

    print("created middle closure")
    return middle
end

a = outer()
b = a()
b()
