use load_values as values_alias
use LOAD_VALUES as values_const_alias

extern "c" fn sink(value: Int)

async fn load_values(value: Int) -> [Int; 3] {
    return [value, value + 1, value + 2]
}

const LOAD_VALUES: (Int) -> Task[[Int; 3]] = load_values

async fn main() -> Int {
    let branch = true
    match await (if branch { values_alias } else { values_const_alias })(30) {
        [first, _, last] if first < last => sink(first + last),
        _ => sink(0),
    }
    match await (match branch { true => values_const_alias, false => values_alias })(13) {
        [first, middle, last] if first == 13 => sink(first + middle + last),
        _ => sink(0),
    }
    return 0
}
