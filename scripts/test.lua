
n=1
function b(h) print 'b'..n n=h print 'c'..n end

function a() print 'e'..n  n=3 print 'f'..n b(10) print 'g'..n end

print 'a'..n
b()
print 'd'..n
a(5,5)
print 'h'.. n