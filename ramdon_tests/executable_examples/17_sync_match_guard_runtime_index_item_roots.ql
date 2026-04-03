use LIMITS as INPUT

const VALUES: [Int; 3] = [1, 3, 5]
static LIMITS: [Int; 3] = [2, 4, 6]

struct State {
    offset: Int,
}

fn main() -> Int {
    let index = 0
    let state = State { offset: 1 }
    let first = match 0 {
        0 if VALUES[index + 1] == 3 => 10,
        _ => 0,
    }
    let second = match 0 {
        0 if INPUT[state.offset] == 4 => 12,
        _ => 0,
    }
    let third = match 0 {
        0 if LIMITS[index + state.offset + 1] == 6 => 20,
        _ => 0,
    }
    return first + second + third
}
