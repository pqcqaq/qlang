use LIMIT as THRESHOLD

const LIMIT: Int = 1

fn main() -> Int {
    let value = 2
    return match value {
        2 if value > THRESHOLD => 20,
        _ => 0,
    }
}
