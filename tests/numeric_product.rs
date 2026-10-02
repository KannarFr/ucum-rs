//! A numeric factor in a product must not change how the other factors are evaluated.

use octofhir_ucum::precision::{NumericOps, from_f64};
use octofhir_ucum::{
    Dimension, EvalResult, UnitExpr, UnitFactor, evaluate, evaluate_owned, parse_expression,
};

fn eval(expr: &str) -> EvalResult {
    let ast = parse_expression(expr).expect("parse ok");
    evaluate_owned(&ast).expect("eval ok")
}

fn assert_factor(result: &EvalResult, expected: f64, input: &str) {
    let tolerance = from_f64(expected.abs() * 1e-9);
    assert!(
        (result.factor.sub(from_f64(expected))).abs() <= tolerance,
        "{input}: factor is {}, expected {expected}",
        result.factor
    );
}

#[test]
fn prefix_is_kept_next_to_a_number() {
    for (input, factor) in [
        ("2.km", 2_000.0),
        ("km.2", 2_000.0),
        ("2.mg", 0.002),
        ("10.kPa", 10_000_000.0), // Pa = 1000 g.m-1.s-2
        ("2.km2", 2_000_000.0),
        ("2.m", 2.0),
        ("2.[in_i]", 0.0508),
        ("2.km.s", 2_000.0),
        ("2.km/s", 2_000.0),
        ("2.mm.km", 2.0),
    ] {
        assert_factor(&eval(input), factor, input);
    }
}

#[test]
fn number_scales_the_unit_it_multiplies() {
    for (input, unit) in [("2.km", "km"), ("3.mL", "mL"), ("5.ug", "ug")] {
        let with_number = eval(input);
        let alone = eval(unit);
        assert_eq!(with_number.dim, alone.dim, "{input}");
    }
    assert_eq!(eval("2.km").dim, Dimension([0, 1, 0, 0, 0, 0, 0]));
}

#[test]
fn unit_starting_with_a_prefix_letter_is_found() {
    // "mol" must not be read as milli + "ol"
    let mol = eval("mol");
    let two_mol = eval("2.mol");
    assert_eq!(two_mol.dim, mol.dim);
    assert_eq!(two_mol.factor, mol.factor.mul(from_f64(2.0)));
}

#[test]
fn special_units_keep_their_meaning() {
    // 2 B is a ratio of 10^2, 2 Np a ratio of e^2: the number is the argument, not a multiplier
    assert_factor(&eval("2.B"), 100.0, "2.B");
    assert_factor(&eval("2.Np"), core::f64::consts::E.powi(2), "2.Np");
}

#[test]
fn borrowed_symbols_keep_their_dimension() {
    // `evaluate` on a hand-built AST uses `UnitExpr::Symbol`, the parser uses `SymbolOwned`
    let product = UnitExpr::Product(vec![
        UnitFactor {
            expr: UnitExpr::Numeric(2.0),
            exponent: 1,
        },
        UnitFactor {
            expr: UnitExpr::Symbol("km"),
            exponent: 1,
        },
    ]);
    let result = evaluate(&product).expect("eval ok");
    assert_eq!(result.dim, Dimension([0, 1, 0, 0, 0, 0, 0]));
    assert_factor(&result, 2_000.0, "2.km (borrowed)");
}
