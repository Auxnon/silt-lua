
function thrice(fn)
    for i = 1, 3 do
        fn(i)
    end
end

thrice(function(i) print(i) end)