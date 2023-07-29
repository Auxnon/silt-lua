function a()
    return function()
        print "a"
    end
end

a()()