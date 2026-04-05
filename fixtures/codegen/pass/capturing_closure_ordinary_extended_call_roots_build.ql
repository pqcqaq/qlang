fn main() -> Int {
    let branch = true
    let target = 42
    let run = (value: Int) => value + target
    var alias = run
    let direct_assignment = (alias = run)(1)
    let direct_control_flow_assignment = (if branch { alias = run } else { run })(2)
    let direct_block_local = (match branch {
        true => {
            var local = run
            local = run;
            local
        },
        false => run,
    })(3)
    let chosen = if branch {
        let local = run
        local
    } else {
        run
    }
    return direct_assignment + direct_control_flow_assignment + direct_block_local + chosen(4)
}
