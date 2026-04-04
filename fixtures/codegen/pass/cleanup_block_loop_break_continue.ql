extern "c" fn stop() -> Bool
extern "c" fn step()
extern "c" fn after()

fn main() -> Int {
    defer {
        loop {
            if stop() {
                break
            };
            step();
            continue;
            after();
        }
    }
    return 0
}
