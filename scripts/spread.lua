gui = {
	fill = function(self, a, b)
		print(a, b)
	end,
}
fill = function(...)
	gui:fill(...)
end
