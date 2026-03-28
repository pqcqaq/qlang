fn helper() -> Int {
    let flag = true
    return match flag {
        true => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
