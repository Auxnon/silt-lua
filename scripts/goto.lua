b = true
::a::
print(a)
do
    local a = 5
    print(a)
    if b then
        b = false
        goto a
    end
    print(a)
end
