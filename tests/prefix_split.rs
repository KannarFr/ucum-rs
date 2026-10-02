//! A prefixed code is a prefix followed by a unit code, whatever the length of the prefix.

use octofhir_ucum::precision::NumericOps;
use octofhir_ucum::{ErrorKind, evaluate_owned, parse_expression};

fn factor(expr: &str) -> f64 {
    let ast = parse_expression(expr).expect("parse ok");
    evaluate_owned(&ast).expect("eval ok").factor.to_f64()
}

fn assert_ratio(prefixed: &str, unit: &str, expected: f64) {
    let ratio = factor(prefixed) / factor(unit);
    assert!(
        (ratio - expected).abs() <= expected * 1e-9,
        "{prefixed} / {unit} is {ratio}, expected {expected}"
    );
}

#[test]
fn deca_is_not_read_as_deci() {
    for unit in ["m", "g", "L", "Bq", "bar"] {
        assert_ratio(&format!("da{unit}"), unit, 10.0);
    }
}

#[test]
fn deci_still_works() {
    for unit in ["m", "g", "L", "ar"] {
        assert_ratio(&format!("d{unit}"), unit, 0.1);
    }
}

#[test]
fn binary_prefixes_work() {
    assert_ratio("KiBy", "By", 1024.0);
    assert_ratio("MiBy", "By", 1024.0 * 1024.0);
    assert_ratio("GiBy", "By", 1024.0 * 1024.0 * 1024.0);
    assert_ratio("Tibit", "bit", 1024.0f64.powi(4));
}

#[test]
fn unit_code_wins_over_prefix_split() {
    // "Pa" is pascal, not peta + annum; "cd" is candela, not centi + day
    assert_ratio("Pa", "g/m/s2", 1_000.0);
    assert_eq!(factor("cd"), 1.0);
}

#[test]
fn double_prefix_is_rejected() {
    for input in ["kmg", "mkm", "dadam"] {
        let ast = parse_expression(input).expect("parse ok");
        let err = evaluate_owned(&ast).unwrap_err();
        assert!(
            matches!(err.kind, ErrorKind::UnitNotFound { .. }),
            "{input}: unexpected error {:?}",
            err.kind
        );
    }
}
