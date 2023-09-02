# Benchmarks

Benchmarks are via Criterion. As can be seen, `Trc`'s performance really shines when there are many Clones.
The reason `Trc` does not do as well for fewer operations is that it needs to allocate `n+1` blocks of memory for `n` threads, and
so for 1 thread, there are 2 allocations. However, after the initial allocations, `Trc` performs very well - 3.81x `Arc`'s time for Clones.
### Clone   
| Type | Mean time |
| --- | ----------- |
| Trc | 26.913ns |
| Arc | 33.803ns |
| Rc | 11.228ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 423.020ns |
| Arc | 1273.200ns |
| Rc | 352.920ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 20.802ns |
| Arc | 20.802ns |
| Rc | 9.264ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 35.577ns |
| Arc | 35.853ns |
| Rc | 29.454ns |

### Multiple Threads Drop and Clone (1000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 2.80ms |
| Arc | 4.02ms |

1.44x faster - because of the allocation cost of `SharedTrc`.

### Multiple Threads Drop and Clone (5000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 3.480ms |
| Arc | 9.415ms |

2.71x faster - the allocation cost of `SharedTrc` begins to become replaced by the `Clone` efficiency.

### Multiple Threads Drop and Clone (100000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 18.451ms |
| Arc | 137.980ms |

7.44x faster - the allocation cost of `SharedTrc` is now negligible and the `Clone` advantage is beginning to be demonstrated.

### Multiple Threads Drop and Clone (500000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 71.490ms |
| Arc | 638.180ms |

8.92x faster - the allocation cost of `SharedTrc` is now negligible and the `Clone` advantage is demonstrated.

Test platforms:
- WSL2 5.15.90 + Intel Xeon E3-1535M (x86_64)
- Ubuntu 20.04 + Intel Xeon Platinum 8358 (x86_64)
- Debian 6.1.20 + ARM Neoverse-N1 (aarch64)
- MacOS 13.4.1 + Apple M1 Max (aarch64)

## WSL2 (Ubuntu 22.04) 5.15.90 + Intel Xeon E3-1535M (x86_64)

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
git clone https://github.com/EricLBuehler/trc.git
cd trc
# Benchmarks
cargo install cargo-criterion
cargo criterion
# Tests
rustup +nightly component add miri
cargo +nightly miri test
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test
MIRIFLAGS="-Zmiri-symbolic-alignment-check" cargo +nightly miri test
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

<span style="color:green">**All tests passed.**</span>

### Clone
| Type | Mean time |
| --- | ----------- |
| Trc | 36.201ns |
| Arc | 43.793ns |
| Rc | 14.112ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 350.000ns |
| Arc | 1331.900ns |
| Rc | 303.73ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 26.978ns |
| Arc | 24.745ns |
| Rc | 12.249ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 55.148ns |
| Arc | 54.261ns |
| Rc | 44.764ns |

### Multiple Threads Drop and Clone (1000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 10.945ms |
| Arc | 13.754ms |

### Multiple Threads Drop and Clone (5000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 12.300ms |
| Arc | 18.104ms |

### Multiple Threads Drop and Clone (100000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 30.666ms |
| Arc | 126.330ms |

### Multiple Threads Drop and Clone (500000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 88.387ms |
| Arc | 534.67ms |

![Trc vs Arc performance (WSL2 x86_64)](./figures/wsl2_x86_64.png)


## Ubuntu 20.04 + Intel Xeon Platinum 8358 (x86_64)

```shell
sudo apt update
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
git clone https://github.com/EricLBuehler/trc.git
cd trc
# Benchmarks
cargo install cargo-criterion
cargo criterion
# Tests
rustup +nightly component add miri
cargo +nightly miri test
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test
MIRIFLAGS="-Zmiri-symbolic-alignment-check" cargo +nightly miri test
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

<span style="color:green">**All tests passed.**</span>

### Clone   
| Type | Mean time |
| --- | ----------- |
| Trc | 26.913ns |
| Arc | 33.803ns |
| Rc | 11.228ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 423.020ns |
| Arc | 1273.200ns |
| Rc | 352.920ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 20.802ns |
| Arc | 20.802ns |
| Rc | 9.264ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 35.577ns |
| Arc | 35.853ns |
| Rc | 29.454ns |

### Multiple Threads Drop and Clone (1000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 2.80ms |
| Arc | 4.02ms |

### Multiple Threads Drop and Clone (5000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 3.480ms |
| Arc | 9.415ms |

### Multiple Threads Drop and Clone (100000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 18.451ms |
| Arc | 137.980ms |

### Multiple Threads Drop and Clone (500000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 71.490ms |
| Arc | 638.180ms |

![Trc vs Arc performance (Ubuntu x86_64)](./figures/ubuntu_x86_64.png)


## Debian 6.1.20 + ARM Neoverse-N1 (aarch64)

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
git clone https://github.com/EricLBuehler/trc.git
cd trc
sudo apt install git
sudo apt install build-essential
# Benchmarks
cargo install cargo-criterion
cargo criterion
# Tests
rustup +nightly component add miri
cargo +nightly miri test
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test
MIRIFLAGS="-Zmiri-symbolic-alignment-check" cargo +nightly miri test
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

<span style="color:green">**All tests passed.**</span>


### Clone
| Type | Mean time |
| --- | ----------- |
| Trc | 45.005ns |
| Arc | 42.873ns |
| Rc | 16.684ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 422.680ns |
| Arc | 1208.100ns |
| Rc | 353.41ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 28.721ns |
| Arc | 32.815ns |
| Rc | 14.134ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 62.983ns |
| Arc | 61.937ns |
| Rc | 48.643ns |

### Multiple Threads Drop and Clone (1000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 1.883ms |
| Arc | 2.913ms |

### Multiple Threads Drop and Clone (5000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 2.424ms |
| Arc | 7.672ms |

### Multiple Threads Drop and Clone (100000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 16.535ms |
| Arc | 119.950ms |

### Multiple Threads Drop and Clone (500000 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 71.543ms |
| Arc | 590.530ms |

![Trc vs Arc performance (Debian aarch64)](./figures/debian_aarch64.png)


## MacOS 13.4.1 + Apple M1 Max (aarch64)

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
git clone https://github.com/EricLBuehler/trc.git
cd trc
sudo apt install git
sudo apt install build-essential
# Benchmarks
cargo install cargo-criterion
cargo criterion
# Tests
rustup +nightly component add miri
cargo +nightly miri test
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test
MIRIFLAGS="-Zmiri-symbolic-alignment-check" cargo +nightly miri test
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

<span style="color:green">**All tests passed.**</span>


### Clone
| Type | Mean time |
| --- | ----------- |
| Trc | 29.750ns |
| Arc | 20.221ns |
| Rc | 16.191ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 440.81ns |
| Arc | 969.18ns |
| Rc | 409.55ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 16.279ns |
| Arc | 16.620ns |
| Rc | 15.590ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 49.710ns |
| Arc | 49.918ns |
| Rc | 46.284ns |

### Multiple Threads Drop and Clone (1000 times)
| Type | Mean time |
| --- |-----------|
| Trc | 1.688ms   |
| Arc | 2.668ms   |

### Multiple Threads Drop and Clone (5000 times)
| Type | Mean time |
| --- |-----------|
| Trc | 1.968ms   |
| Arc | 6.488ms   |

### Multiple Threads Drop and Clone (100000 times)
| Type | Mean time |
| --- | ---------- |
| Trc | 8.095ms |
| Arc | 97.976ms |

### Multiple Threads Drop and Clone (500000 times)
| Type | Mean time |
| --- |----------|
| Trc | 34.526ms |
| Arc | 477.230ms |

![Trc vs Arc performance (apple darwin aarch64)](./figures/apple_darwin_aarch64.png)