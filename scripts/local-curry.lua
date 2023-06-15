function a()
    local b= function()
        print "a"
    end
    return b
end

a()()
b()