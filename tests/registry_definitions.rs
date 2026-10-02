//! Every regular unit of the generated registry must agree with its definition in
//! `ucum-essence.xml`: `<value Unit="EXPR" value="V">` means "V times EXPR".
//!
//! The expected factor and dimension are computed with the runtime parser and evaluator,
//! which is a different code path from the one `build.rs` uses to generate the registry.

#![allow(clippy::result_large_err)]

use octofhir_ucum::precision::to_f64;
use octofhir_ucum::{evaluate_owned, find_unit, parse_expression};
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};

/// Definitions the runtime parser and evaluator do not handle yet (negative exponents, a
/// leading solidus followed by another one, the `'` symbol, left-to-right evaluation of
/// `a/b.c`, values below the `Decimal` range). They are checked by hand below.
const NOT_EVALUABLE: &[&str] = &["Hz", "S", "Bq", "Ky", "[G]", "Oe", "''", "[cml_i]", "b"];

struct Definition {
    code: String,
    value: f64,
    expr: String,
}

fn attribute(tag: &BytesStart, name: &str) -> Option<String> {
    tag.attributes()
        .filter_map(Result::ok)
        .find(|a| a.key.as_ref() == name.as_bytes())
        .map(|a| {
            a.normalized_value(XmlVersion::Implicit1_0)
                .expect("valid attribute")
                .into_owned()
        })
}

/// Read the definition of every unit that is not marked `isSpecial`.
fn regular_definitions() -> Vec<Definition> {
    let xml = std::fs::read_to_string("ucum-essence.xml").expect("read ucum-essence.xml");
    let mut reader = Reader::from_str(&xml);
    let mut definitions = Vec::new();
    let mut current: Option<String> = None;

    loop {
        match reader.read_event().expect("valid XML") {
            Event::Start(tag) if tag.name().as_ref() == b"unit" => {
                let special = attribute(&tag, "isSpecial").as_deref() == Some("yes");
                current = (!special).then(|| attribute(&tag, "Code").expect("unit code"));
            }
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"value" => {
                if let Some(code) = current.take() {
                    definitions.push(Definition {
                        code,
                        value: attribute(&tag, "value")
                            .expect("value")
                            .parse()
                            .expect("numeric value"),
                        expr: attribute(&tag, "Unit").expect("Unit"),
                    });
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    definitions
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

#[test]
fn registry_matches_unit_definitions() {
    let definitions = regular_definitions();
    assert!(
        definitions.len() > 250,
        "only {} definitions",
        definitions.len()
    );

    let mut mismatches = Vec::new();
    for Definition { code, value, expr } in &definitions {
        // The mole has its own dimension in this crate; UCUM defines it as a pure number.
        if code == "mol" || NOT_EVALUABLE.contains(&code.as_str()) {
            continue;
        }
        let unit = find_unit(code)
            .filter(|unit| unit.code == code)
            .unwrap_or_else(|| panic!("{code} is missing from the registry"));
        let definition = parse_expression(expr)
            .and_then(|ast| evaluate_owned(&ast))
            .unwrap_or_else(|e| panic!("{code}: cannot evaluate `{expr}`: {e}"));
        let factor = value * to_f64(definition.factor);

        if unit.dim != definition.dim || !close(unit.factor, factor) {
            mismatches.push(format!(
                "{code} = {value} {expr}: registry has {:e} {:?}, definition gives {factor:e} {:?}",
                unit.factor, unit.dim.0, definition.dim.0
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn units_checked_by_hand() {
    let per_second = [0, 0, -1, 0, 0, 0, 0];
    for code in ["Hz", "Bq"] {
        let unit = find_unit(code).unwrap();
        assert_eq!(unit.dim.0, per_second, "{code}");
        assert_eq!(unit.factor, 1.0, "{code}");
    }

    // S = Ohm-1, and Ohm = 1000 g.m2.s-3.A-2
    let siemens = find_unit("S").unwrap();
    assert_eq!(siemens.dim.0, [-1, -2, 3, 2, 0, 0, 0]);
    assert!(close(siemens.factor, 1e-3));

    // Ky = cm-1
    let kayser = find_unit("Ky").unwrap();
    assert_eq!(kayser.dim.0, [0, -1, 0, 0, 0, 0, 0]);
    assert!(close(kayser.factor, 100.0));

    // [G] = 6.6743e-11 m3.kg-1.s-2
    let big_g = find_unit("[G]").unwrap();
    assert_eq!(big_g.dim.0, [-1, 3, -2, 0, 0, 0, 0]);
    assert!(close(big_g.factor, 6.6743e-14));

    // Oe = 250 /[pi].A/m
    let oersted = find_unit("Oe").unwrap();
    assert_eq!(oersted.dim.0, [0, -1, 0, 1, 0, 0, 0]);
    assert!(close(oersted.factor, 250.0 / std::f64::consts::PI));

    // '' = '/60, ' = deg/60, deg = 2 [pi].rad/360
    let arc_second = find_unit("''").unwrap();
    assert_eq!(arc_second.dim.0, [0; 7]);
    assert!(close(arc_second.factor, std::f64::consts::PI / 648_000.0));

    // [cml_i] = [pi]/4.[mil_i]2, evaluated left to right; [mil_i] = 2.54e-5 m
    let circular_mil = find_unit("[cml_i]").unwrap();
    assert_eq!(circular_mil.dim.0, [0, 2, 0, 0, 0, 0, 0]);
    assert!(close(
        circular_mil.factor,
        std::f64::consts::FRAC_PI_4 * 2.54e-5 * 2.54e-5
    ));

    // b = 100 fm2
    let barn = find_unit("b").unwrap();
    assert_eq!(barn.dim.0, [0, 2, 0, 0, 0, 0, 0]);
    assert!(close(barn.factor, 1e-28));
}

#[test]
fn common_units_have_their_documented_value() {
    // Factors are relative to the base units g, m, s
    for (code, factor) in [
        ("%", 1e-2),
        ("[ppm]", 1e-6),
        ("L", 1e-3),
        ("l", 1e-3),
        ("[in_i]", 0.0254),
        ("[ft_us]", 1200.0 / 3937.0),
        ("[oz_av]", 28.349523125),
        ("[gal_us]", 0.003785411784),
        ("[psi]", 6_894_757.293168361),
        ("eV", 1.602176634e-16),
        ("By", 8.0),
    ] {
        let unit = find_unit(code).unwrap();
        assert!(
            close(unit.factor, factor),
            "{code}: registry has {:e}, expected {factor:e}",
            unit.factor
        );
    }
}
