use ping as Op

fn ping(value: Int) -> Int {
    return value
}

fn main() -> Int {
    let direct = ping.scope(true)
    let alias = Op.scope(true)
    return 0
}
