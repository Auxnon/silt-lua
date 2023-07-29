function fib(n)
  if n <= 1 then
    return n
  else
  return fib(n-1) + fib(n-2)
  end
end

for i = 1, 100 do
    print(i..":"..fib(i))
end