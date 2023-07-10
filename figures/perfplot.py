import matplotlib.pyplot as plt

plt.plot([1000, 5000, 100000, 500000], [10.945, 12.300, 30.666, 88.387], color = "red")
plt.plot([1000, 5000, 100000, 500000], [13.754, 18.104, 126.330, 534.67], color = "blue")
plt.ylabel("Benchmarked time (ms)")
plt.xlabel("Number of iterations")
plt.title("Arc vs Trc performance")
plt.legend(("Trc","Arc"), shadow=True, fancybox=True)
plt.show()