struct Pair {
    left: Int,
    right: Int,
}

extern "c" fn first()
extern "c" fn second()
extern "c" fn sink(value: Int)

const PICK_LEFT: Bool = true
const PICK_VALUES: Int = 0

fn main() -> Int {
    defer {
        let Pair { left, right } = if PICK_LEFT {
            Pair { left: 4, right: 6 }
        } else {
            Pair { left: 8, right: 10 }
        };
        sink(left);
        sink(right);
        for value in match PICK_VALUES {
            0 => [12, 14],
            _ => [16, 18],
        } {
            sink(value);
        }
        if match PICK_VALUES {
            0 => true,
            _ => false,
        } {
            first();
        } else {
            second();
        };
    }
    return 0
}
