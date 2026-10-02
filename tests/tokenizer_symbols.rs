//! Units and annotations that the tokenizer used to drop or misread.

use octofhir_ucum::precision::NumericOps;
use octofhir_ucum::{
    ErrorKind, OwnedUnitExpr, evaluate_owned, find_unit, parse_expression, validate,
};

fn sym(s: &str) -> OwnedUnitExpr {
    OwnedUnitExpr::Symbol(s.to_string())
}

fn factor(expr: &str) -> f64 {
    let ast = parse_expression(expr).expect("parse ok");
    evaluate_owned(&ast).expect("eval ok").factor.to_f64()
}

#[test]
fn units_starting_with_a_symbol_character_are_units() {
    // These used to parse as the empty expression, i.e. the number 1
    for code in ["'", "''", "%"] {
        assert_eq!(parse_expression(code).unwrap(), sym(code));
        let registry = find_unit(code).expect("unit exists").factor;
        assert!((factor(code) - registry).abs() <= registry * 1e-9, "{code}");
    }
    assert_eq!(
        parse_expression("'/60").unwrap(),
        OwnedUnitExpr::Quotient(Box::new(sym("'")), Box::new(OwnedUnitExpr::Numeric(60.0)))
    );
}

#[test]
fn ten_power_without_exponent_is_ten() {
    for code in ["10*", "10^"] {
        assert_eq!(
            parse_expression(code).unwrap(),
            OwnedUnitExpr::Numeric(10.0)
        );
    }
    assert_eq!(factor("10*.m"), 10.0);
}

#[test]
fn micro_sign_units_take_exponents() {
    assert_eq!(
        parse_expression("µm2").unwrap(),
        OwnedUnitExpr::Power(Box::new(sym("um")), 2)
    );
    assert_eq!(
        parse_expression("µm2").unwrap(),
        parse_expression("um2").unwrap()
    );
    assert_eq!(
        parse_expression("µm-1").unwrap(),
        parse_expression("um-1").unwrap()
    );
}

#[test]
fn standalone_annotation_is_the_unity() {
    for input in ["{rbc}", "{request}", "{a}.rad2{b}"] {
        assert!(validate(input).is_ok(), "{input}");
        assert_eq!(factor(input), 1.0, "{input}");
    }
    assert_eq!(factor("{rbc}/uL"), factor("/uL"));
}

#[test]
fn non_ascii_annotation_is_rejected() {
    // Official functional test 1-115a
    let err = parse_expression("rad2{錠}").unwrap_err();
    assert!(matches!(err.kind, ErrorKind::InvalidExpression { .. }));
    assert!(parse_expression("{é}").is_err());
}
