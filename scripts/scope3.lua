-- test upper scope variable usage in function call
do
    local a = 4
    function f()
        print("print" .. a)
    end

    f()
end
