function p(a)
    print(a.."?")
end

function var(...)
    local t=...
    p(t)
end

for i=1,10 do
    var(i)
end
