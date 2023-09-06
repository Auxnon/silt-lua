n = 12
i = 2
for i = 2, n, i do
    while n % i == 0 do
        print(i)
        n = n / i
    end
end
