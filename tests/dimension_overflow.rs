//! Dimension exponents are stored as `i8`. An exponent outside that range must be reported
//! as an error instead of wrapping around or saturating.

#![allow(clippy::result_large_err)]

use octofhir_ucum::{
    Dimension, ErrorKind, UcumError, divide_by, evaluate_owned, multiply, parse_expression,
    unit_divide, unit_multiply,
};

fn assert_overflow<T: std::fmt::Debug>(result: Result<T, UcumError>, what: &str) {
    let err = result.expect_err(what);
    assert!(
        matches!(err.kind, ErrorKind::PrecisionOverflow { .. }),
        "{what}: unexpected error {:?}",
        err.kind
    );
}

fn eval_dim(input: &str) -> Result<Dimension, UcumError> {
    let ast = parse_expression(input)?;
    evaluate_owned(&ast).map(|result| result.dim)
}

#[test]
fn exponent_out_of_range_is_an_error() {
    // These used to give the dimensions -56, -128, 44 and 0
    for input in ["m200", "m128", "m300", "m2000000000"] {
        assert_overflow(eval_dim(input), input);
    }
}

#[test]
fn accumulated_exponent_out_of_range_is_an_error() {
    // 64 + 64 used to saturate at 127
    for input in ["m64.m64", "m100.m100", "m100/(/m100)", "(m64)2"] {
        assert_overflow(eval_dim(input), input);
    }
}

#[test]
fn largest_exponents_still_work() {
    assert_eq!(
        eval_dim("m127").unwrap(),
        Dimension([0, 127, 0, 0, 0, 0, 0])
    );
    assert_eq!(
        eval_dim("/m127").unwrap(),
        Dimension([0, -127, 0, 0, 0, 0, 0])
    );
    assert_eq!(
        eval_dim("m64.m63").unwrap(),
        Dimension([0, 127, 0, 0, 0, 0, 0])
    );
}

#[test]
fn quantity_and_unit_arithmetic_check_the_range() {
    assert_overflow(multiply(1.0, "m100", 1.0, "m100"), "multiply");
    assert_overflow(divide_by(1.0, "m100", 1.0, "/m100"), "divide_by");
    assert_overflow(unit_multiply("m100", "m100"), "unit_multiply");
    assert_overflow(unit_divide("m100", "/m100"), "unit_divide");
}
