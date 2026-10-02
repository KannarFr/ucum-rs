//! An operator must be followed by a unit, and the whole input must be consumed.

use octofhir_ucum::{ErrorKind, parse_expression, validate};

fn assert_invalid(input: &str) {
    let err = parse_expression(input).expect_err(input);
    assert!(
        matches!(err.kind, ErrorKind::InvalidExpression { .. }),
        "{input}: unexpected error {:?}",
        err.kind
    );
    assert!(validate(input).is_err(), "validate({input:?})");
}

#[test]
fn operator_without_operand_is_rejected() {
    // Official functional test 1-102: "/ is not followed by a term"
    for input in ["m/", "m.", "m/s/", "/", ".", "m..s", "m//s", "()"] {
        assert_invalid(input);
    }
}

#[test]
fn trailing_input_is_rejected() {
    // These used to evaluate as if the tail were not there
    for input in ["(m)-2", "10-3", "m -2", "m-"] {
        assert!(validate(input).is_err(), "validate({input:?})");
    }
    assert_invalid("(m)-2");
    assert_invalid("10-3");
}

#[test]
fn valid_expressions_still_parse() {
    for input in [
        "/min", "m/(s)", "(m)", " m ", "kg.m/s2", "{a}/m", "m.{a}", "10*-7", "m-2",
    ] {
        assert!(parse_expression(input).is_ok(), "{input}");
    }
}
