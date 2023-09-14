-- for loop closures should capture the loop variable at that point in time and not the final value of the loop variable
a = {}
do
    for i = 1, 3 do
        local function t()
            return i
        end
        print(i)
        a[i] = t
    end

    return a[1]() + a[2]() + a[3]() -- 1+2+3 = 6
end
