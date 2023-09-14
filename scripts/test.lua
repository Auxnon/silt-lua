start = clock()
i = 1
a = "a"
while i < 100000 do
    i = i + 1
    a = a .. "1"
end
elapsed = clock() - start
print "done "
print("elapsed: " .. elapsed)
return { elapsed, i }
