use LIMIT as Bound
use TOTAL as Count

const LIMIT: Int = 1
static TOTAL: Int = 2

fn main(value: Int) -> Int {
    match value {
        LIMIT => 1,
        Bound => 2,
        TOTAL => 3,
        Count => 4,
        _ => 0,
    }
}
