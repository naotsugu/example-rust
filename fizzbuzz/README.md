# FizzBuzz


When a function returns `()`, the return type can be omitted from the signature.

```rust
fn fizzbuzz_to(n: u32) {
    for n in 1..=n {
        fizzbuzz(n);
    }
}
```


Functions that "don't" return a value, actually return the unit type `()`.

```rust
fn fizzbuzz(n: u32) -> () {
    if is_divisible_by(n, 15) {
        println!("fizzbuzz");
    } else if is_divisible_by(n, 3) {
        println!("fizz");
    } else if is_divisible_by(n, 5) {
        println!("buzz");
    } else {
        println!("{}", n);
    }
}

```


Function that returns a boolean value.

```rust
fn is_divisible_by(lhs: u32, rhs: u32) -> bool {
    if rhs == 0 {
        return false;
    }
    lhs % rhs == 0
}
```


```rust
fn main() {
    fizzbuzz_to(100);
}
```



## Build

```shell
$ cargo build
```

## Run

```shell
$ cargo run
1
2
fizz
4
buzz
fizz
7
8
fizz
buzz
11
fizz
13
14
fizzbuzz
16
17
fizz
19
buzz
```