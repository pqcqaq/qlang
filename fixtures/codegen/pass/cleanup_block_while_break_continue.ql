extern "c" fn running() -> Bool
extern "c" fn stop() -> Bool
extern "c" fn step()
extern "c" fn after()

fn main() -> Int {
    defer {
        while running() {
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
