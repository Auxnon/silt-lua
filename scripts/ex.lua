do
    local a = 1
    local b = 2
    print("sum of " .. a .. " + " .. b .. " = " .. a + b)
    print("int or float types " .. 1 + 2.5 / 3)
    print("string inferance works, '2'+2=" .. "2" + 2)
    local function closure()
        local c = 0
        local function nested()
            c = c + a
            return "closures work " .. c
        end

        return nested
    end

    local iterate = closure()
    print(iterate())
    print(iterate())
    print("You can also return values to the console.")
    return "Completed!"
end
