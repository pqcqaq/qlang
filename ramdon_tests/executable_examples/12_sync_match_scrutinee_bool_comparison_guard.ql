use ENABLE as ON

const ENABLE: Bool = true

fn choose(flag: Bool) -> Int {
    return match flag {
        true if flag == ON => 42,
        false => 0,
    }
}

fn main() -> Int {
    return choose(true)
}
