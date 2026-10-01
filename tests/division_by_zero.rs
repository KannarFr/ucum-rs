//! A zero denominator must be reported as an error, never a panic.

use octofhir_ucum::{ErrorKind, evaluate_owned, parse_expression, validate};

#[test]
fn zero_denominator_is_an_error() {
    for input in ["m/0", "1/0", "(1/0)", "m/(0.s)", "m/0/s"] {
        let ast = parse_expression(input).expect("parse ok");
        let err = evaluate_owned(&ast).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::DivisionByZero),
            "{input}: unexpected error {:?}",
            err.kind
        );
        assert!(validate(input).is_err(), "validate({input:?})");
    }
}

#[test]
fn zero_numerator_is_still_valid() {
    let ast = parse_expression("0/m").expect("parse ok");
    assert!(evaluate_owned(&ast).is_ok());
}
