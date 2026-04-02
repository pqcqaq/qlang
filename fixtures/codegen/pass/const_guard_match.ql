const ENABLE: Bool = true
const DISABLE: Bool = false

fn main() -> Int {
    let value = 2
    return match value {
        1 if DISABLE => 10,
        2 if ENABLE => 20,
        other if ENABLE => other,
    }
}
