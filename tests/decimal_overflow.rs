//! A `Decimal` operation that overflows must be reported as an error, never a panic.

use octofhir_ucum::{
    ErrorKind, UnitExpr, UnitFactor, evaluate, evaluate_owned, parse_expression, validate,
};

fn eval_kind(input: &str) -> ErrorKind {
    let ast = parse_expression(input).expect("parse ok");
    evaluate_owned(&ast).unwrap_err().kind
}

#[test]
fn power_overflow_is_an_error() {
    // (10^24 m)^2 and (10^3 m)^10 do not fit in a `Decimal`
    for input in ["Ym2", "Ym3", "km10", "km30"] {
        let kind = eval_kind(input);
        assert!(
            matches!(kind, ErrorKind::PrecisionOverflow { .. }),
            "{input}: unexpected error {kind:?}"
        );
        assert!(validate(input).is_err(), "validate({input:?})");
    }
}

#[test]
fn product_overflow_is_an_error() {
    for input in ["Ym.Ym", "10*20.10*20", "Ym/ym"] {
        let kind = eval_kind(input);
        assert!(
            matches!(kind, ErrorKind::PrecisionOverflow { .. }),
            "{input}: unexpected error {kind:?}"
        );
    }
}

#[test]
fn huge_exponent_fails_without_looping() {
    // Used to multiply two billion times before overflowing.
    let kind = eval_kind("km2000000000");
    assert!(
        matches!(kind, ErrorKind::PrecisionOverflow { .. }),
        "unexpected error {kind:?}"
    );
}

#[test]
fn zero_raised_to_a_negative_power_is_a_division_by_zero() {
    let power = UnitExpr::Power(Box::new(UnitExpr::Numeric(0.0)), -1);
    assert!(matches!(
        evaluate(&power).unwrap_err().kind,
        ErrorKind::DivisionByZero
    ));

    let product = UnitExpr::Product(vec![
        UnitFactor {
            expr: UnitExpr::Numeric(0.0),
            exponent: -1,
        },
        UnitFactor {
            expr: UnitExpr::Symbol("m"),
            exponent: 1,
        },
    ]);
    assert!(matches!(
        evaluate(&product).unwrap_err().kind,
        ErrorKind::DivisionByZero
    ));
}

#[test]
fn largest_representable_power_is_exact() {
    let ast = parse_expression("km9").expect("parse ok");
    let result = evaluate_owned(&ast).expect("eval ok");
    assert_eq!(result.factor.to_string(), "1000000000000000000000000000");
}
