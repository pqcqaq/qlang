use INDEX as SLOT

const INDEX: Int = 0
static NEXT: Int = 1

fn main() -> Int {
    var pair = (1, 2)
    let first = pair[SLOT] = 7
    let second = pair[NEXT] = first + 6
    return first + second
}
