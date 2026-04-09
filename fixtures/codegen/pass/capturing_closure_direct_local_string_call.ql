const TARGET: String = "alpha"

fn main() -> Int {
    let captured = TARGET
    let run = () => if captured == TARGET { 41 } else { 0 }
    return run()
}
