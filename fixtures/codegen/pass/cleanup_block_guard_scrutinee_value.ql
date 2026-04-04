extern "c" fn note()
extern "c" fn first()
extern "c" fn second()
extern "c" fn sink(value: Int)

fn enabled() -> Bool {
    return true
}

fn main() -> Int {
    let flag = true
    defer if {
        note();
        enabled()
    } {
        match {
            note();
            flag
        } {
            true => sink({
                note();
                1
            }),
            false => second(),
        }
    } else {
        first()
    }
    return 0
}
