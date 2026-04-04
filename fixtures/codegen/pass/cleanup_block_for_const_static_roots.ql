use MORE as alias_more
use HOLDER as alias_holder

struct Holder {
    values: [Int; 3],
}

extern "c" fn sink(value: Int)

const VALUES: [Int; 3] = [1, 2, 3]
static MORE: [Int; 3] = [4, 5, 6]
const HOLDER: Holder = Holder { values: [7, 8, 9] }

fn main() -> Int {
    defer {
        for value in VALUES {
            sink(value)
        }
        for item in alias_more {
            sink(item)
        }
        for projected in alias_holder.values {
            sink(projected)
        }
    }
    return 0
}
