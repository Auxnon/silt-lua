
local a="global"
do
    function f1()
        print(a)
    end

    f1()
    local a="block"
    f1()
end