do
  function fib(n)
    if n <= 1 then
      return n
    else
      return fib(n - 1) + fib(n - 2)
    end
  end

  local i = 1
  while i < 100 do
    print(i .. ":" .. fib(i))
    i = i + 1
  end
end
