import matplotlib.pyplot as plt

plt.plot([1000, 5000, 100000, 500000], [2.8, 3.48, 18.451, 71.49], color = "red")
plt.plot([1000, 5000, 100000, 500000], [4.02, 9.415, 137.980, 680.18], color = "blue")
plt.ylabel("Benchmarked time (ms)")
plt.xlabel("Number of iterations")
plt.title("Arc vs Trc performance")
plt.legend(("Trc","Arc"), shadow=True, fancybox=True)
plt.show()