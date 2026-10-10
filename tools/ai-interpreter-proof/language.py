import native_probe
class Number:
    def __init__(self,value): self.value=value
    def twice(self): return self.value*2
counter=Number(sum(i*i for i in range(10)))
print(counter.twice())
assert native_probe.add(3,4)==7
