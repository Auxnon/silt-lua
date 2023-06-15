
function a(n)
    print(n)
    if n<=1 then
        return n
    end
    return a(n-1)
end

print("final:"..a(20))