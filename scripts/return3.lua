function a(n)
    print(n)
    do
        print("a")
        if n <= 1 then
            return n
        end
        print("b")
    end
    print("c")
end


a(2)
a(1)
return "done"