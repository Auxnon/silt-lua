        -- function test(...)
        --      local a,b=...
        --     return a
        -- end
        -- return test(1,3,7,12)

        function test(a)
            -- local a,b=5,8
            print(a)
            return a
        end
        return test "yes"
