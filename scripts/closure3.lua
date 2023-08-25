do
    local a = "global"
    function f1()
        print(a)
    end

    -- local b = "2"

    f1()
    a = "block"
    f1()
    -- a = "2"
end
