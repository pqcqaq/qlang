mod support;

use support::diagnostic_messages;

#[test]
fn accepts_direct_closures_for_callable_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: (Int) -> Int, value: Int) -> Int {
    return f(value)
}

fn main() -> Int {
    return apply((x) => x + 1, 2)
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_block_closures_with_explicit_return_when_callable_return_matches() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn main() -> Int {
    return apply(() => {
        return 1
    })
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn closure_block_returns_are_checked_against_callable_return_type() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: () -> String) -> String {
    return f()
}

fn main() -> Int {
    let value = apply(() => {
        return 1
    })
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"return value has type mismatch: expected `String`, found `Int`".to_string()
        ),
        "expected closure return mismatch diagnostic, got {diagnostics:?}"
    );
    assert!(
        !diagnostics.contains(
            &"closure body has type mismatch: expected `String`, found `Void`".to_string()
        ),
        "did not expect fallback closure-body void mismatch, got {diagnostics:?}"
    );
}

#[test]
fn nested_closure_returns_do_not_force_outer_closure_callable_return() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn main() -> Int {
    let value = apply(() => {
        let inner = () => {
            return 1
        }
    })
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `() -> Int`, found `() -> Void`"
                .to_string()
        ),
        "expected outer closure callable mismatch, got {diagnostics:?}"
    );
}

#[test]
fn reports_function_bodies_that_can_fall_through_without_returning() {
    let diagnostics = diagnostic_messages(
        r#"
fn maybe(flag: Bool) -> Int {
    if flag {
        return 1
    }
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"function body has type mismatch: expected `Int`, found `Void`".to_string()),
        "expected missing function return diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_bodies_whose_if_expression_returns_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn choose(flag: Bool) -> Int {
    if flag {
        return 1
    } else {
        return 2
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_matches_with_catch_all_returning_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn choose(flag: Bool) -> Int {
    match flag {
        true => {
            return 1
        }
        _ => {
            return 2
        }
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_matches_over_bool_when_both_cases_return() {
    let diagnostics = diagnostic_messages(
        r#"
fn choose(flag: Bool) -> Int {
    match flag {
        true => {
            return 1
        }
        false => {
            return 0
        }
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_guarded_bool_matches_as_non_exhaustive_for_function_returns() {
    let diagnostics = diagnostic_messages(
        r#"
fn choose(flag: Bool) -> Int {
    match flag {
        true if flag => {
            return 1
        }
        false => {
            return 0
        }
    }
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"function body has type mismatch: expected `Int`, found `Void`".to_string()),
        "expected missing function return diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_matches_over_enum_when_all_variants_return() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Quit,
    Value(Int),
    Config {
        retries: Int,
    },
}

fn run(command: Command) -> Int {
    match command {
        Command.Quit => {
            return 0
        }
        Command.Value(value) => {
            return value
        }
        Command.Config { retries } => {
            return retries
        }
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_matches_over_import_aliased_enum_when_all_variants_return() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Quit,
    Value(Int),
}

fn run(command: Command) -> Int {
    match command {
        Cmd.Quit => {
            return 0
        }
        Cmd.Value(value) => {
            return value
        }
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_closure_bodies_whose_bool_match_returns_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: (Bool) -> Int, flag: Bool) -> Int {
    return f(flag)
}

fn main() -> Int {
    return apply((flag) => match flag {
        true => {
            return 1
        }
        false => {
            return 0
        }
    }, true)
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_function_bodies_with_loop_statements_that_return_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn run() -> Int {
    loop {
        return 1
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_loop_bodies_with_break_before_return_as_non_returning() {
    let diagnostics = diagnostic_messages(
        r#"
fn run() -> Int {
    loop {
        break
        return 1
    }
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"function body has type mismatch: expected `Int`, found `Void`".to_string()),
        "expected missing function return diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_returns_after_non_breaking_loops_as_non_returning() {
    let diagnostics = diagnostic_messages(
        r#"
fn run() -> Int {
    loop {
    }
    return 1
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"function body has type mismatch: expected `Int`, found `Void`".to_string()),
        "expected unreachable trailing return to stay outside guaranteed-return analysis, got {diagnostics:?}"
    );
}

#[test]
fn accepts_closure_bodies_with_loop_statements_that_return_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: () -> Int) -> Int {
    return f()
}

fn main() -> Int {
    return apply(() => {
        loop {
            return 1
        }
    })
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_closure_bodies_that_can_fall_through_without_returning() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: (Bool) -> Int, flag: Bool) -> Int {
    return f(flag)
}

fn main() -> Int {
    let value = apply((flag) => {
        if flag {
            return 1
        }
    }, true)
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"closure body has type mismatch: expected `Int`, found `Void`".to_string()),
        "expected missing closure return diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn accepts_closure_bodies_whose_if_expression_returns_on_all_paths() {
    let diagnostics = diagnostic_messages(
        r#"
fn apply(f: (Bool) -> Int, flag: Bool) -> Int {
    return f(flag)
}

fn main() -> Int {
    return apply((flag) => if flag {
        return 1
    } else {
        return 2
    }, true)
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_tuple_multi_return_destructuring() {
    let diagnostics = diagnostic_messages(
        r#"
fn div_rem(left: Int, right: Int) -> (Int, Int) {
    return (left / right, left % right)
}

fn main() -> Int {
    let (q, r) = div_rem(10, 3)
    return q + r
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_struct_literals_that_use_default_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
    age: Int = 0,
}

fn make(name: String) -> User {
    return User { name }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_return_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    return "oops"
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"return value has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_non_bool_conditions() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    while 1 {
        break
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"while condition must have type `Bool`, found `Int`".to_string())
    );
}

#[test]
fn reports_break_and_continue_outside_loops() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    break
    continue
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`break` is only allowed inside loop bodies".to_string()),
        "expected break-outside-loop diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`continue` is only allowed inside loop bodies".to_string()),
        "expected continue-outside-loop diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn allows_break_and_continue_inside_loop_bodies() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    loop {
        break
    }

    while true {
        continue
    }

    for value in [1, 2, 3] {
        break
    }

    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn closures_do_not_inherit_loop_control_contexts() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    loop {
        let stop = () => {
            break
        }
        let skip = () => {
            continue
        }
        break
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`break` is only allowed inside loop bodies".to_string()),
        "expected closure break diagnostic, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"`continue` is only allowed inside loop bodies".to_string()),
        "expected closure continue diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_tuple_pattern_arity_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let (left, right, extra) = (1, 2)
    return left + right
}
"#,
    );

    assert!(diagnostics.contains(&"tuple pattern expects 3 item(s), found 2".to_string()));
}

#[test]
fn reports_tuple_pattern_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let (left, right) = 1
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"tuple pattern requires a tuple value, found `Int`".to_string()));
}

#[test]
fn reports_call_arity_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return add(1)
}
"#,
    );

    assert!(diagnostics.contains(&"expected 2 argument(s), found 1".to_string()));
}

#[test]
fn reports_call_argument_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return add(1, "x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_call_argument_type_mismatches_through_const_callable_values() {
    let diagnostics = diagnostic_messages(
        r#"
const APPLY: (Int) -> Int = (value) => value + 1

fn main() -> Int {
    return APPLY("x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_call_argument_type_mismatches_through_imported_function_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use add as plus

fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return plus(1, "x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_named_call_argument_type_mismatches_through_imported_function_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use add as plus

fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return plus(left: 1, right: true)
}
"#,
    );

    assert!(diagnostics.contains(
        &"named call argument has type mismatch: expected `Int`, found `Bool`".to_string()
    ));
}

#[test]
fn reports_call_argument_type_mismatches_through_imported_const_callable_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use APPLY as run

const APPLY: (Int) -> Int = (value) => value + 1

fn main() -> Int {
    return run("x")
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Int`, found `String`".to_string()
        )
    );
}

#[test]
fn reports_call_argument_type_mismatches_for_extern_block_functions() {
    let diagnostics = diagnostic_messages(
        r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(true, 2)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string())
    );
}

#[test]
fn reports_non_callable_values() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    return value()
}
"#,
    );

    assert!(diagnostics.contains(&"cannot call value of type `Int`".to_string()));
}

#[test]
fn reports_non_callable_import_alias_values() {
    let diagnostics = diagnostic_messages(
        r#"
use VALUE as current

const VALUE: Int = 1

fn main() -> Int {
    return current()
}
"#,
    );

    assert!(diagnostics.contains(&"cannot call value of type `Int`".to_string()));
}

#[test]
fn reports_invalid_member_access_on_non_item_values() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    value.name
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"member access is not supported on type `Int`".to_string()));
}

#[test]
fn invalid_member_receivers_do_not_reuse_root_function_signatures_for_calls() {
    let diagnostics = diagnostic_messages(
        r#"
use ping as Op

fn ping(value: Int) -> Int {
    return value
}

fn main() -> Int {
    let direct = ping.scope(true)
    let alias = Op.scope(true)
    return 0
}
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|message| {
                message == &&"member access is not supported on type `(Int) -> Int`".to_string()
            })
            .count(),
        2,
        "expected both invalid member receivers to be diagnosed, got {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("call argument has type mismatch")),
        "invalid member receivers should not reuse root function signatures for deeper path calls, got {diagnostics:?}"
    );
}

#[test]
fn reports_invalid_index_access_on_non_indexable_values() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    value[0]
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"indexing is not supported on type `Int`; only arrays and tuples are indexable"
                .to_string()
        )
    );
}

#[test]
fn accepts_array_literals_and_array_indexing() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let values = [1, 2, 3]
    return values[0] + values[1]
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_assignment_to_mutable_bindings() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    var (left, right) = (1, 2)
    left = right
    return left
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_assignment_to_mutable_receiver_self() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn replace(var self, next: Counter) -> Counter {
        self = next
        return self
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_assignment_to_immutable_local_bindings() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let value = 1
    value = 2
    return value
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"cannot assign to immutable local `value`; declare it with `var`".to_string()
        )
    );
}

#[test]
fn reports_assignment_to_immutable_parameters() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(value: Int) -> Int {
    value = 2
    return value
}
"#,
    );

    assert!(diagnostics.contains(&"cannot assign to immutable parameter `value`".to_string()));
}

#[test]
fn reports_assignment_to_immutable_receiver_self() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn replace(self, next: Counter) -> Counter {
        self = next
        return self
    }
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"cannot assign to immutable receiver `self`; use `var self`".to_string())
    );
}

#[test]
fn reports_assignment_to_top_level_values() {
    let diagnostics = diagnostic_messages(
        r#"
const limit: Int = 1
static total: Int = 0

fn main() -> Int {
    limit = 2;
    total = 3
    return total
}
"#,
    );

    assert!(diagnostics.contains(&"cannot assign to constant `limit`".to_string()));
    assert!(diagnostics.contains(&"cannot assign to static `total`".to_string()));
}

#[test]
fn reports_assignment_to_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    add = add
    return add(1, 2)
}
"#,
    );

    assert!(diagnostics.contains(&"cannot assign to function `add`".to_string()));
}

#[test]
fn reports_assignment_to_imported_bindings() {
    let diagnostics = diagnostic_messages(
        r#"
use source_value as source

const source_value: Int = 1

fn main() -> Int {
    source = 2
    return source_value
}
"#,
    );

    assert!(diagnostics.contains(&"cannot assign to imported binding `source`".to_string()));
}

#[test]
fn reports_unsupported_member_assignment_targets() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

fn main() -> Int {
    var counter = Counter { value: 1 }
    counter.value = 2
    return counter.value
}
"#,
    );

    assert!(diagnostics.contains(
        &"assignment through member access is not supported yet; only bare mutable bindings can be assigned"
            .to_string()
    ));
}

#[test]
fn reports_unsupported_index_assignment_targets() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    var values = [1, 2, 3]
    values[0] = 4
    return values[0]
}
"#,
    );

    assert!(diagnostics.contains(
        &"assignment through indexing is not supported yet; only bare mutable bindings can be assigned"
            .to_string()
    ));
}

#[test]
fn accepts_declared_array_types_in_function_signatures() {
    let diagnostics = diagnostic_messages(
        r#"
fn take_first(values: [Int; 3]) -> Int {
    return values[0]
}

fn main() -> Int {
    return take_first([1, 2, 3])
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_tuple_indexing_with_integer_literals() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let pair = (1, "ql")
    let first = pair[0]
    return first
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_tuple_indexing_with_hexadecimal_integer_literals() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let pair = (1, 2)
    let second = pair[0x1]
    return second
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_array_literal_item_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let values = [1, "x"]
    return 0
}
"#,
    );

    assert!(diagnostics.contains(
        &"array literal item has type mismatch: expected `Int`, found `String`".to_string()
    ));
}

#[test]
fn reports_array_literal_item_type_mismatches_against_declared_array_types() {
    let diagnostics = diagnostic_messages(
        r#"
fn take(values: [Int; 2]) -> Int {
    return values[0]
}

fn main() -> Int {
    return take(["x", "y"])
}
"#,
    );

    assert!(diagnostics.contains(
        &"array literal item has type mismatch: expected `Int`, found `String`".to_string()
    ));
}

#[test]
fn reports_declared_array_length_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn take_first(values: [Int; 3]) -> Int {
    return values[0]
}

fn main() -> Int {
    return take_first([1, 2])
}
"#,
    );

    assert!(diagnostics.contains(
        &"call argument has type mismatch: expected `[Int; 3]`, found `[Int; 2]`".to_string()
    ));
}

#[test]
fn reports_non_int_array_indices() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let values = [1, 2, 3]
    return values["x"]
}
"#,
    );

    assert!(diagnostics.contains(&"array index must have type `Int`, found `String`".to_string()));
}

#[test]
fn reports_tuple_index_out_of_bounds() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    let pair = (1, 2)
    return pair[2]
}
"#,
    );

    assert!(
        diagnostics.contains(&"tuple index `2` is out of bounds for tuple of length 2".to_string())
    );
}

#[test]
fn keeps_dynamic_tuple_indexing_deferred() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(index: Int) -> Int {
    let pair = (1, 2)
    pair[index]
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "dynamic tuple indexing should stay deferred for now, got {diagnostics:?}"
    );
}

#[test]
fn reports_unknown_struct_members() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

fn main() -> Int {
    let user = User { name: "ql" }
    user.age
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"unknown member `age` on type `User`".to_string()));
}

#[test]
fn reports_ambiguous_impl_method_selection() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn ping(self) -> Int {
        return self.value
    }
}

impl Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.ping()
}
"#,
    );

    assert!(diagnostics.contains(
        &"ambiguous method `ping` on type `Counter`; multiple matching methods found".to_string()
    ));
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("cannot call value of type"))
    );
}

#[test]
fn reports_ambiguous_extend_method_selection() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

extend Counter {
    fn ping(self) -> Int {
        return self.value
    }
}

extend Counter {
    fn ping(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main(counter: Counter) -> Int {
    return counter.ping()
}
"#,
    );

    assert!(diagnostics.contains(
        &"ambiguous method `ping` on type `Counter`; multiple matching methods found".to_string()
    ));
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("cannot call value of type"))
    );
}

#[test]
fn accepts_method_selection_without_field_false_positives() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn get(self) -> Int {
        return self.value
    }

    fn next(self) -> Int {
        return self.get()
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_method_call_argument_type_mismatches_for_unique_member_selection() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn add(self, delta: Int) -> Int {
        return self.value + delta
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.add(true)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string())
    );
}

#[test]
fn prefers_impl_methods_over_extend_candidates() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self, delta: Int) -> Int {
        return self.value + delta
    }
}

extend Counter {
    fn read(self) -> Int {
        return self.value
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    return counter.read(true)
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"call argument has type mismatch: expected `Int`, found `Bool`".to_string()),
        "expected impl method to win over extend candidate, got {diagnostics:?}"
    );
}

#[test]
fn deferred_multi_segment_member_targets_do_not_attach_to_concrete_local_items() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter.Scope.Config {
    fn read(self) -> Int {
        return 1
    }
}

extend Counter.Scope.Config {
    fn extra(self) -> Int {
        return 1
    }
}

fn main(counter: Counter) -> Int {
    return counter.read() + counter.extra()
}
"#,
    );

    assert!(
        diagnostics.contains(&"unknown member `read` on type `Counter`".to_string()),
        "expected deferred impl target to stay detached from concrete local item, got {diagnostics:?}"
    );
    assert!(
        diagnostics.contains(&"unknown member `extra` on type `Counter`".to_string()),
        "expected deferred extend target to stay detached from concrete local item, got {diagnostics:?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|message| message.contains("ambiguous method")),
        "expected detached deferred targets instead of fake method matches, got {diagnostics:?}"
    );
}

#[test]
fn reports_struct_literal_shape_and_field_type_errors() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
    age: Int = 0,
}

fn main() -> User {
    return User { age: "old", missing: 1 }
}
"#,
    );

    assert!(diagnostics.contains(
        &"struct literal field has type mismatch: expected `Int`, found `String`".to_string()
    ));
    assert!(diagnostics.contains(&"unknown field `missing` in struct literal".to_string()));
    assert!(diagnostics.contains(&"missing required field `name` in struct literal".to_string()));
}

#[test]
fn reports_invalid_struct_literal_roots() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Config {
        retries: Int,
    },
    Value(Int),
}

fn main() -> Int {
    let builtin_value = Int { value: 1 }
    let enum_value = Command { value: 1 }
    let tuple_variant = Command.Value { value: 1 }
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"struct literal syntax is not supported for `Int`".to_string()));
    assert!(
        diagnostics.contains(&"struct literal syntax is not supported for `Command`".to_string())
    );
    assert!(
        diagnostics
            .contains(&"struct literal syntax is not supported for `Command.Value`".to_string())
    );
}

#[test]
fn reports_invalid_struct_literal_roots_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Int {
    let value = Cmd { retries: 1 }
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"struct literal syntax is not supported for `Cmd`".to_string()));
}

#[test]
fn reports_invalid_generic_struct_literal_roots() {
    let diagnostics = diagnostic_messages(
        r#"
fn build[T]() -> Int {
    let value = T { field: 1 }
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"struct literal syntax is not supported for `T`".to_string()));
}

#[test]
fn accepts_struct_literals_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use User as Person

struct User {
    name: String,
    age: Int = 0,
}

fn main() -> User {
    return Person { name: "ql" }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_struct_literal_shape_errors_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use User as Person

struct User {
    name: String,
    age: Int = 0,
}

fn main() -> User {
    return Person { age: "old", missing: 1 }
}
"#,
    );

    assert!(diagnostics.contains(
        &"struct literal field has type mismatch: expected `Int`, found `String`".to_string()
    ));
    assert!(diagnostics.contains(&"unknown field `missing` in struct literal".to_string()));
    assert!(diagnostics.contains(&"missing required field `name` in struct literal".to_string()));
}

#[test]
fn accepts_enum_struct_variant_literals() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Config {
        retries: Int,
        enabled: Bool,
    },
}

fn main() -> Command {
    return Command.Config { retries: 3, enabled: false }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_enum_struct_variant_literals_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
        enabled: Bool,
    },
}

fn main() -> Command {
    return Cmd.Config { retries: 3, enabled: false }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_enum_struct_variant_literal_shape_and_field_type_errors() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Config {
        retries: Int,
        name: String,
        enabled: Bool,
    },
}

fn main() -> Command {
    return Command.Config { retries: "old", missing: 1 }
}
"#,
    );

    assert!(diagnostics.contains(
        &"struct literal field has type mismatch: expected `Int`, found `String`".to_string()
    ));
    assert!(diagnostics.contains(&"unknown field `missing` in struct literal".to_string()));
    assert!(diagnostics.contains(&"missing required field `name` in struct literal".to_string()));
}

#[test]
fn reports_enum_struct_variant_literal_shape_errors_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
        name: String,
        enabled: Bool,
    },
}

fn main() -> Command {
    return Cmd.Config { retries: "old", missing: 1 }
}
"#,
    );

    assert!(diagnostics.contains(
        &"struct literal field has type mismatch: expected `Int`, found `String`".to_string()
    ));
    assert!(diagnostics.contains(&"unknown field `missing` in struct literal".to_string()));
    assert!(diagnostics.contains(&"missing required field `name` in struct literal".to_string()));
}

#[test]
fn reports_unknown_enum_struct_variant_literals() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Command {
    return Command.Missing { retries: 3 }
}
"#,
    );

    assert_eq!(
        diagnostics,
        vec!["unknown variant `Missing` in enum `Command`".to_string()]
    );
}

#[test]
fn reports_unknown_enum_struct_variant_literals_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Command {
    return Cmd.Missing { retries: 3 }
}
"#,
    );

    assert_eq!(
        diagnostics,
        vec!["unknown variant `Missing` in enum `Command`".to_string()]
    );
}

#[test]
fn keeps_deeper_enum_struct_literal_paths_deferred() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Command {
    let direct = Command.Scope.Config { retries: 3 }
    return Cmd.Scope.Missing { retries: 4 }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "deeper enum-like struct literal paths should stay deferred for now, got {diagnostics:?}"
    );
}

#[test]
fn invalid_struct_literal_roots_do_not_cascade_into_return_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Value(Int),
}

fn main() -> Bool {
    return Command.Value { value: 1 }
}
"#,
    );

    assert_eq!(
        diagnostics,
        vec!["struct literal syntax is not supported for `Command.Value`".to_string()]
    );
}

#[test]
fn deferred_struct_literal_roots_do_not_cascade_into_return_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
}

fn main() -> Bool {
    return Cmd.Scope.Config { retries: 1 }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "deferred struct literal roots should not force return mismatches, got {diagnostics:?}"
    );
}

#[test]
fn deferred_multi_segment_import_alias_types_do_not_canonicalize_to_local_items() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Config {
        retries: Int,
    },
}

fn expects(value: Cmd.Scope.Config) -> Int {
    return 0
}

fn returns(value: Command) -> Cmd.Scope.Config {
    return value
}

fn main(command: Command) -> Int {
    return expects(command)
}
"#,
    );

    assert!(diagnostics.contains(
        &"return value has type mismatch: expected `Cmd.Scope.Config`, found `Command`".to_string()
    ));
    assert!(
        diagnostics.contains(
            &"call argument has type mismatch: expected `Cmd.Scope.Config`, found `Command`"
                .to_string()
        )
    );
}

#[test]
fn reports_pattern_root_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

struct Point {
    x: Int,
}

fn main() -> Int {
    let Point { x } = User { name: "ql" }
    return x
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"struct pattern has type mismatch: expected `User`, found `Point`".to_string()
        )
    );
}

#[test]
fn accepts_struct_patterns_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Point as P

struct Point {
    x: Int,
    y: Int,
}

fn main(point: Point) -> Int {
    match point {
        P { x, y } => x + y,
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_unknown_struct_pattern_fields() {
    let diagnostics = diagnostic_messages(
        r#"
struct Point {
    x: Int,
}

fn main(point: Point) -> Int {
    match point {
        Point { missing } => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(&"unknown field `missing` in struct pattern".to_string()));
}

#[test]
fn reports_pattern_root_type_mismatches_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Point as P

struct User {
    name: String,
}

struct Point {
    x: Int,
}

fn main() -> Int {
    let P { x } = User { name: "ql" }
    return x
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"struct pattern has type mismatch: expected `User`, found `Point`".to_string()
        )
    );
}

#[test]
fn reports_unknown_struct_pattern_fields_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Point as P

struct Point {
    x: Int,
}

fn main(point: Point) -> Int {
    match point {
        P { missing } => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(&"unknown field `missing` in struct pattern".to_string()));
}

#[test]
fn reports_invalid_pattern_roots() {
    let diagnostics = diagnostic_messages(
        r#"
struct Point {
    x: Int,
}

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
    Empty,
}

fn main(point: Point, result: Result) -> Int {
    let Point(x) = point
    let Result.Named(named_value) = result
    let Result.Value { value: tuple_value } = result
    let Result { value: root_value } = result
    let Result.Empty { value: empty_value } = result
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"tuple-struct pattern syntax is not supported for `Point`".to_string())
    );
    assert!(
        diagnostics.contains(
            &"tuple-struct pattern syntax is not supported for `Result.Named`".to_string()
        )
    );
    assert!(
        diagnostics
            .contains(&"struct pattern syntax is not supported for `Result.Value`".to_string())
    );
    assert!(
        diagnostics.contains(&"struct pattern syntax is not supported for `Result`".to_string())
    );
    assert!(
        diagnostics
            .contains(&"struct pattern syntax is not supported for `Result.Empty`".to_string())
    );
}

#[test]
fn reports_invalid_pattern_roots_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Point as P
use Result as Res

struct Point {
    x: Int,
}

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
}

fn main(point: Point, result: Result) -> Int {
    let P(x) = point
    let Res.Named(named_value) = result
    let Res.Value { value: tuple_value } = result
    let Res { value: root_value } = result
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"tuple-struct pattern syntax is not supported for `P`".to_string())
    );
    assert!(
        diagnostics
            .contains(&"tuple-struct pattern syntax is not supported for `Res.Named`".to_string())
    );
    assert!(
        diagnostics.contains(&"struct pattern syntax is not supported for `Res.Value`".to_string())
    );
    assert!(diagnostics.contains(&"struct pattern syntax is not supported for `Res`".to_string()));
}

#[test]
fn accepts_unit_variant_path_patterns() {
    let diagnostics = diagnostic_messages(
        r#"
enum Command {
    Quit,
    Value(Int),
}

fn main(command: Command) -> Int {
    match command {
        Command.Quit => 1,
        _ => 0,
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn accepts_unit_variant_path_patterns_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Command as Cmd

enum Command {
    Quit,
    Value(Int),
}

fn main(command: Command) -> Int {
    match command {
        Cmd.Quit => 1,
        _ => 0,
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_invalid_path_pattern_roots() {
    let diagnostics = diagnostic_messages(
        r#"
struct Point {
    x: Int,
}

enum Result {
    Empty,
    Value(Int),
    Named {
        value: Int,
    },
}

fn main(point: Point, result: Result) -> Int {
    let Point = point
    let Result = result
    let Result.Value = result
    let Result.Named = result
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"path pattern syntax is not supported for `Point`".to_string()));
    assert!(diagnostics.contains(&"path pattern syntax is not supported for `Result`".to_string()));
    assert!(
        diagnostics
            .contains(&"path pattern syntax is not supported for `Result.Value`".to_string())
    );
    assert!(
        diagnostics
            .contains(&"path pattern syntax is not supported for `Result.Named`".to_string())
    );
}

#[test]
fn reports_invalid_path_pattern_roots_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Point as P
use Result as Res

struct Point {
    x: Int,
}

enum Result {
    Empty,
    Value(Int),
    Named {
        value: Int,
    },
}

fn main(point: Point, result: Result) -> Int {
    let P = point
    let Res = result
    let Res.Value = result
    let Res.Named = result
    return 0
}
"#,
    );

    assert!(diagnostics.contains(&"path pattern syntax is not supported for `P`".to_string()));
    assert!(diagnostics.contains(&"path pattern syntax is not supported for `Res`".to_string()));
    assert!(
        diagnostics.contains(&"path pattern syntax is not supported for `Res.Value`".to_string())
    );
    assert!(
        diagnostics.contains(&"path pattern syntax is not supported for `Res.Named`".to_string())
    );
}

#[test]
fn reports_unsupported_const_static_path_patterns() {
    let diagnostics = diagnostic_messages(
        r#"
const LIMIT: Int = 1
static TOTAL: Int = 2

fn main(value: Int) -> Int {
    match value {
        LIMIT => 1,
        TOTAL => 2,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(&"path pattern syntax is not supported for `LIMIT`".to_string()));
    assert!(diagnostics.contains(&"path pattern syntax is not supported for `TOTAL`".to_string()));
}

#[test]
fn reports_unsupported_const_static_path_patterns_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use LIMIT as Bound
use TOTAL as Count

const LIMIT: Int = 1
static TOTAL: Int = 2

fn main(value: Int) -> Int {
    match value {
        Bound => 1,
        Count => 2,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(&"path pattern syntax is not supported for `Bound`".to_string()));
    assert!(diagnostics.contains(&"path pattern syntax is not supported for `Count`".to_string()));
}

#[test]
fn reports_unknown_enum_variant_patterns() {
    let diagnostics = diagnostic_messages(
        r#"
enum Result {
    Named {
        value: Int,
    },
    Value(Int),
    Empty,
}

fn main(result: Result) -> Int {
    let Result.Missing(value) = result
    let Result.Other { value } = result
    let Result.Unknown = result
    return 0
}
"#,
    );

    assert_eq!(
        diagnostics,
        vec![
            "unknown variant `Missing` in enum `Result`".to_string(),
            "unknown variant `Other` in enum `Result`".to_string(),
            "unknown variant `Unknown` in enum `Result`".to_string(),
        ]
    );
}

#[test]
fn reports_unknown_enum_variant_patterns_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Result as Res

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
    Empty,
}

fn main(result: Result) -> Int {
    let Res.Missing(value) = result
    let Res.Other { value } = result
    let Res.Unknown = result
    return 0
}
"#,
    );

    assert_eq!(
        diagnostics,
        vec![
            "unknown variant `Missing` in enum `Result`".to_string(),
            "unknown variant `Other` in enum `Result`".to_string(),
            "unknown variant `Unknown` in enum `Result`".to_string(),
        ]
    );
}

#[test]
fn keeps_deeper_enum_pattern_paths_deferred() {
    let diagnostics = diagnostic_messages(
        r#"
use Result as Res

enum Result {
    Named {
        value: Int,
    },
    Value(Int),
    Empty,
}

fn main(result: Result) -> Int {
    let Result.Scope.Value(value) = result
    let Res.Scope.Named { value } = result
    let Res.Scope.Empty = result
    return 0
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "deeper enum-like pattern paths should stay deferred for now, got {diagnostics:?}"
    );
}

#[test]
fn reports_variant_pattern_type_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

enum Result {
    Ok(Int),
    Err(String),
}

fn main() -> Int {
    let user = User { name: "ql" }
    match user {
        Result.Ok(value) => value,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(
        &"tuple-struct pattern has type mismatch: expected `User`, found `Result`".to_string()
    ));
}

#[test]
fn accepts_variant_struct_patterns_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Result as Res

enum Result {
    Ok {
        value: Int,
    },
    Err {
        code: Int,
    },
}

fn main(result: Result) -> Int {
    match result {
        Res.Ok { value } => value,
        Res.Err { code } => code,
    }
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_unknown_variant_struct_pattern_fields_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Result as Res

enum Result {
    Ok {
        value: Int,
    },
    Err {
        code: Int,
    },
}

fn main(result: Result) -> Int {
    match result {
        Res.Ok { missing } => 0,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(&"unknown field `missing` in struct pattern".to_string()));
}

#[test]
fn reports_variant_pattern_type_mismatches_through_same_file_import_aliases() {
    let diagnostics = diagnostic_messages(
        r#"
use Result as Res

struct User {
    name: String,
}

enum Result {
    Ok(Int),
    Err(String),
}

fn main() -> Int {
    let user = User { name: "ql" }
    match user {
        Res.Ok(value) => value,
        _ => 0,
    }
}
"#,
    );

    assert!(diagnostics.contains(
        &"tuple-struct pattern has type mismatch: expected `User`, found `Result`".to_string()
    ));
}

#[test]
fn accepts_comparison_for_compatible_numeric_operands() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(left: Int, right: Int) -> Bool {
    return left < right
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn reports_comparison_operand_mismatches_for_incompatible_numeric_types() {
    let diagnostics = diagnostic_messages(
        r#"
fn main(left: Int, right: UInt) -> Bool {
    return left < right
}
"#,
    );

    assert!(
        diagnostics.contains(
            &"comparison operator `<` expects compatible numeric operands, found `Int` and `UInt`"
                .to_string()
        )
    );
}

#[test]
fn reports_equality_operand_mismatches() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Bool {
    return 1 == "x"
}
"#,
    );

    assert!(diagnostics.contains(
        &"equality operator `==` expects compatible operands, found `Int` and `String`".to_string()
    ));
}
