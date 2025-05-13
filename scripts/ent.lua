function fun()
    local a=test_ent()
    print("we made ent")
    print("it has "..a.x)
    a.x=10
    print("it has "..a.x)
end
fun()
