use add_one as item_alias
use APPLY as const_alias
use APPLY_CLOSURE_CONST as closure_const
use APPLY_CLOSURE_STATIC as closure_static
use worker as async_item_alias
use ASYNC_APPLY as async_const_alias

extern "c" fn sink(value: Int)

fn add_one(value: Int) -> Int {
    return value + 1
}

async fn worker(value: Int) -> Int {
    return value + 10
}

const APPLY: (Int) -> Int = add_one
const APPLY_CLOSURE_CONST: (Int) -> Int = (value: Int) => value + 2
static APPLY_CLOSURE_STATIC: (Int) -> Int = (value: Int) => value + 3
const ASYNC_APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
    let branch = true
    let picked_sync = if branch { item_alias } else { const_alias }
    let picked_closure = match branch {
        true => closure_const,
        false => closure_static,
    }
    let picked_async = if branch { async_item_alias } else { async_const_alias }
    let matched_async = match branch {
        true => async_const_alias,
        false => async_item_alias,
    }
    var total = picked_sync(10) + picked_closure(20)
    total = total + await picked_async(30);
    total = total + await matched_async(40);
    defer {
        let cleanup_sync = if branch { closure_const } else { item_alias }
        sink(cleanup_sync(50));
        let cleanup_async = match branch {
            true => async_item_alias,
            false => async_const_alias,
        }
        sink(await cleanup_async(60));
    }
    return total
}
