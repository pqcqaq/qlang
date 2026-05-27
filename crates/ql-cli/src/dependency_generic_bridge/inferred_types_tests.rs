use super::super::inferred_type_conversion::{
    inferred_type_from_rendered_substitution, type_expr_from_inferred_type,
};
use super::*;

#[test]
fn rendered_substitution_preserves_tuple_and_array_shape() {
    let inferred = inferred_type_from_rendered_substitution("(Int, [Bool; 2])");
    let round_tripped = type_expr_from_inferred_type(&inferred);
    let inferred_again =
        InferredType::from_type_expr(&round_tripped).expect("round-tripped type should render");

    assert_eq!(inferred.rendered, "(Int, [Bool; 2])");
    assert_eq!(inferred_again, inferred);
}

#[test]
fn rendered_substitution_preserves_generic_named_args() {
    let inferred = inferred_type_from_rendered_substitution("std.result.Result[Int, String]");

    assert_eq!(inferred.rendered, "std.result.Result[Int, String]");
    match inferred.kind {
        InferredTypeKind::Named { path, args } => {
            assert_eq!(path, ["std", "result", "Result"]);
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].rendered, "Int");
            assert_eq!(args[1].rendered, "String");
        }
        other => panic!("expected named inferred type, got {other:?}"),
    }
}
