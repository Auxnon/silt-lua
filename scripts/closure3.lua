do
    local a = "global"
    function f1()
        print(a)
    end

    f1()
    a = "block"
    f1()
end
