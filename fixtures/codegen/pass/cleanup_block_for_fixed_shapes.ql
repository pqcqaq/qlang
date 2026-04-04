extern "c" fn stop() -> Bool
extern "c" fn step(value: Int)
extern "c" fn finish(value: Int)

fn main() -> Int {
    defer {
        for value in [1, 2] {
            if stop() {
                break
            };
            step(value);
            continue;
            finish(value);
        }
        for item in (3, 4) {
            step(item);
            break;
            finish(item);
        }
    }
    return 0
}
