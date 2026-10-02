//! UCUM §7.4: "Terms are evaluated from left to right with the period and the solidus having
//! the same operator precedence." So `a/b.c` is `(a/b).c`.

use octofhir_ucum::precision::{NumericOps, from_f64};
use octofhir_ucum::{Dimension, EvalResult, evaluate_owned, parse_expression};

fn eval(expr: &str) -> EvalResult {
    let ast = parse_expression(expr).expect("parse ok");
    evaluate_owned(&ast).expect("eval ok")
}

fn assert_same(a: &str, b: &str) {
    let (left, right) = (eval(a), eval(b));
    assert_eq!(left.dim, right.dim, "{a} vs {b}");
    let tolerance = right.factor.abs().mul(from_f64(1e-12));
    assert!(
        (left.factor.sub(right.factor)).abs() <= tolerance,
        "{a} = {} but {b} = {}",
        left.factor,
        right.factor
    );
}

#[test]
fn product_after_division_multiplies_the_whole_term() {
    assert_same("s/m.mg", "(s/m).mg");
    assert_same("s/m.mg", "s.mg/m");
    assert_same("m/s.kg/A", "((m/s).kg)/A");
    // Official functional test 3-111a: 6.3 s/m.mg = 0.0063 s.m-1.g
    assert_same("s/m.mg", "10*-3.s.m-1.g");
}

#[test]
fn parentheses_override_the_order() {
    assert_same("s/(m.mg)", "s/m/mg");
    assert_eq!(eval("s/(m.mg)").dim, Dimension([-1, -1, 1, 0, 0, 0, 0]));
    assert_eq!(eval("s/m.mg").dim, Dimension([1, -1, 1, 0, 0, 0, 0]));
}

#[test]
fn successive_divisions_are_unchanged() {
    assert_same("mL/min/kg", "mL/(min.kg)");
    assert_same("kg.m/s2", "(kg.m)/s2");
}

#[test]
fn leading_solidus_can_be_followed_by_more_operators() {
    assert_same("/m/s", "1/m/s");
    assert_same("/m.s", "s/m");
    // Definition of the oersted in ucum-essence.xml: 250 /[pi].A/m
    assert_same("/[pi].A/m", "A/[pi]/m");
}
