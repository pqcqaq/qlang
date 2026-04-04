use MORE as ITEMS
use BOX as STATE

struct Holder {
    values: [Int; 2],
}

extern "c" fn step(value: Int)

const VALUES: [Int; 2] = [1, 2]
static MORE: [Int; 2] = [3, 4]
const BOX: Holder = Holder { values: [5, 6] }

async fn main() -> Int {
    defer {
        for await value in VALUES {
            step(value);
        }
        for await item in ITEMS {
            step(item);
        }
        for await projected in STATE.values {
            step(projected);
        }
    }
    return 0
}
