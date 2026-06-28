-- B
function p(a)
    print(a.."?")
end
function var(b,...)
    local t,u=...
    p(t)
end
-- for i=1,10 do
    var(2,6)
-- end
