-- test local scope within for loops
do
    for a = 1, 3 do
        print(a)
    end
    print(a)

    local i = 10
    print(i)
    for i = 1, 3 do
        local d = 3
        local e = 5
        print(i * e)
    end
    print(i)
end
