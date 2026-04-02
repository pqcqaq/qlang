fn helper() -> Int {
    let flag = true
    return match flag {
        true if flag => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
