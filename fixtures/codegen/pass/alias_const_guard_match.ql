use ENABLE as ON
use DISABLE as OFF

const ENABLE: Bool = true
const DISABLE: Bool = false

fn main() -> Int {
    let value = 2
    return match value {
        1 if OFF => 10,
        2 if ON => 20,
        other if ON => other,
    }
}
