fn main() -> Int {
    var pair = (1, 2)
    let first = pair[0] = 7
    let second = pair[1] = first + 5
    return pair[0] + second
}
