package demo.main

use std.io
use std.collections.HashMap as Map

data struct User {
    name: String,
    age: Int = 0,
}

fn apply(f: (Int) -> Int, value: Int) -> Int {
    return f(value)
}

fn div_rem(left: Int, right: Int) -> (Int, Int) {
    let pair = (left / right, left % right)
    return pair
}

fn make_user(name: String) -> User {
    return User { name }
}

fn main() -> Int {
    let user = make_user("Lin")
    let (q, r) = div_rem(10, 3)
    let add = (x) => x + q
    let map = Map[String, Int].new()
    let value = apply(add, r)
    return value + map["missing"]
}
