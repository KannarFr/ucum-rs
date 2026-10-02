//! The candela is a base unit: it carries the luminous intensity dimension.

use octofhir_ucum::{Dimension, find_unit, is_comparable};

#[test]
fn candela_has_the_luminous_intensity_dimension() {
    let cd = find_unit("cd").expect("cd is a unit");
    assert_eq!(cd.dim, Dimension([0, 0, 0, 0, 0, 0, 1]));
}

#[test]
fn candela_is_not_dimensionless() {
    assert!(!is_comparable("cd", "1").unwrap());
    assert!(!is_comparable("cd", "m").unwrap());
}

#[test]
fn lumen_is_candela_steradian() {
    assert!(is_comparable("lm", "cd.sr").unwrap());
    assert!(is_comparable("lx", "cd/m2").unwrap());
}
