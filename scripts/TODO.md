
### multi var
1. assignment x,y= 1,2
2. invalid assignment x,2=1,2
3. invalid assign 2,3=x,y
4. getter x,y = z,w
5. getter expressions x,y=z+z,w*w
5. mixed getter expr -> x,y=z+2/5+y,w+w-5+z
6. really mixed getter expr -> x,y,z=a(),b(5,6),5+c(4)
6. fn returns -> return x,y
7. fn returns mixed -> return x+5,y-x
8. fn returns really mixed -> return a(4,5),b(c(4,6),5)
9. implicit returns -> function test() 5,6 end
