-- Standard Lua applies, for the most part. 
-- See https://github.com/Auxnon/silt-lua for updates.
-- For best performance use local scope. 
-- Stack based VM written in rust.
-- Many compile time errors are not caught yet. 
-- Standard library and meta functions not yet ready.
do
    local a = nil
    local b = 2
    print("Sum of " .. a .. " + " .. b .. " = " .. a + b)
    print("Int or float types " .. 1 + 2.5 / 3)
    print("String inference works, '2'+2=" .. "2" + 2)
    local function closure()
        local c = 0
        local function nested()
            c = c + a
            return "Closures work " .. c
        end
        return nested
    end
    local iterate = closure()
    print(iterate())
    print(iterate())
    print("You can also return values to the console.")
    return "Completed!"
end
