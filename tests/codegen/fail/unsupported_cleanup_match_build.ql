extern "c" fn first()

fn main() -> Int {
    let flag = true
    defer first()
    return match flag {
        true => 1,
        false => 0,
    }
}
