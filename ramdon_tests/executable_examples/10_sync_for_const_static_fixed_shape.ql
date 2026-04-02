use VALUES as INPUT

const VALUES: (Int, Int) = (20, 20)
static EXTRA: [Int; 1] = [2]

fn main() -> Int {
    var total = 0
    for value in INPUT {
        total = total + value
    }
    for value in EXTRA {
        total = total + value
    }
    return total
}
