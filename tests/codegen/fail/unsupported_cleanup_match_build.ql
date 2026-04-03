extern "c" fn first()

fn main() -> Int {
    let flag = true
    let enabled = false
    defer first()
    return match flag {
        true if enabled => 1,
        false => 0,
    }
}
