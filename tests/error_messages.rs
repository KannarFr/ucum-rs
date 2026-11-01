//! A message quotes the input it is about, so a long input made a message as long: a 1 MB
//! unit gave a 1 MB message, and `validate` a 2 MB `Display`, with the context line. A
//! service that returns errors to its clients answered with twice the size of the request.
//! Messages now show the start of a long input and its size; the fields of the kind keep
//! it whole.

use octofhir_ucum::{
    ErrorKind, UcumError, convert, divide_by, get_canonical_units, multiply, unit_multiply,
    validate, validate_in_property,
};

/// The `Display` of every error, with its suggestions and context
fn shown(err: &UcumError) -> usize {
    err.to_string().len()
}

#[test]
fn messages_of_long_inputs_are_bounded() {
    let big = 1_000_000;
    let unit = "x".repeat(big);
    // Valid units, long through their annotation: a long product would overflow the
    // exponent of `m`
    let note = "a".repeat(big);
    let metre = format!("m{{{note}}}");
    let errors = [
        (validate(&unit), "unit_not_found"),
        (validate(&"[".repeat(big)), "invalid_expression"),
        (validate(&format!("{metre}.qq")), "unit_not_found"),
        (validate(&"(".repeat(big)), "nesting_too_deep"),
        (convert(1.0, &unit, "m").map(|_| ()), "unit_not_found"),
        (convert(1.0, "m", &unit).map(|_| ()), "unit_not_found"),
        (convert(1.0, &metre, "s").map(|_| ()), "dimension_mismatch"),
        (convert(1.0, "s", &metre).map(|_| ()), "dimension_mismatch"),
        (
            convert(1.0, &format!("[IU]{{{note}}}"), "m").map(|_| ()),
            "conversion_error",
        ),
        (
            convert(f64::NAN, &metre, &metre).map(|_| ()),
            "non_finite_value",
        ),
        (get_canonical_units(&unit).map(|_| ()), "unit_not_found"),
        (multiply(1.0, &unit, 1.0, "m").map(|_| ()), "unit_not_found"),
        (
            divide_by(1.0, &metre, 0.0, &metre).map(|_| ()),
            "division_by_zero",
        ),
        (unit_multiply(&unit, "m").map(|_| ()), "unit_not_found"),
        (
            validate_in_property("m", &unit).map(|_| ()),
            "invalid_property",
        ),
    ];
    for (result, code) in errors {
        let err = result.unwrap_err();
        assert_eq!(
            err.kind.code(),
            code,
            "{}",
            &err.message[..80.min(err.message.len())]
        );
        let err = &err;
        assert!(
            shown(err) < 2048,
            "{} bytes: {}...",
            shown(err),
            &err.message[..80.min(err.message.len())]
        );
    }
}

#[cfg(feature = "fhir")]
#[test]
fn fhir_messages_of_long_inputs_are_bounded() {
    use octofhir_ucum::fhir::{FhirQuantity, convert_quantity};
    let mut quantity = FhirQuantity::with_ucum_code(1.0, "m");
    quantity.system = Some("x".repeat(1_000_000));
    let err = quantity.to_ucum_quantity().unwrap_err();
    assert!(
        err.to_string().len() < 2048,
        "{} bytes",
        err.to_string().len()
    );
    let quantity = FhirQuantity::with_ucum_code(1.0, &"x".repeat(1_000_000));
    let err = convert_quantity(&quantity, "m").unwrap_err();
    assert!(
        err.to_string().len() < 2048,
        "{} bytes",
        err.to_string().len()
    );
}

#[test]
fn the_fields_keep_the_whole_input() {
    let unit = "x".repeat(1000);
    let err = convert(1.0, &unit, "m").unwrap_err();
    match &err.kind {
        ErrorKind::UnitNotFound { unit: found, .. } => assert_eq!(found, &unit),
        other => panic!("{other:?}"),
    }
    // The message shows its start and its size
    assert_eq!(
        err.message,
        format!("Unknown unit: '{}...' (1000 bytes)", "x".repeat(64))
    );
    // Long through its annotation; a long product would overflow the exponent of `m`
    let from = format!("m{{{}}}", "a".repeat(1000));
    let err = convert(1.0, &from, "s").unwrap_err();
    match &err.kind {
        ErrorKind::DimensionMismatch {
            from: f,
            to,
            operation,
            ..
        } => {
            assert_eq!(
                (f.as_deref(), to.as_deref()),
                (Some(from.as_str()), Some("s"))
            );
            assert_eq!(operation, &format!("conversion from '{from}' to 's'"));
        }
        other => panic!("{other:?}"),
    }
    // Each unit is shortened on its own: the target and the size of the source remain
    assert_eq!(
        err.message,
        format!(
            "Dimension mismatch in conversion from 'm{{{}...' (1003 bytes) to 's': expected T, found L",
            "a".repeat(62)
        )
    );
    // The same for operands joined into one value
    let err = divide_by(1.0, &from, 0.0, "s").unwrap_err();
    match &err.kind {
        ErrorKind::DivisionByZero { value, .. } => {
            assert_eq!(value, &format!("1.0 {from} / 0.0 s"))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        err.message,
        format!(
            "Division by zero during division: 1.0 m{{{}... (1003 bytes) / 0.0 s",
            "a".repeat(62)
        )
    );
}

#[test]
fn short_inputs_are_shown_whole() {
    assert_eq!(validate("xyz").unwrap_err().message, "Unknown unit: 'xyz'");
    let name = "x".repeat(64);
    assert_eq!(
        validate(&name).unwrap_err().message,
        format!("Unknown unit: '{name}'")
    );
    // Non-ASCII input is cut on a character
    let err = validate_in_property("m", &"é".repeat(100)).unwrap_err();
    assert_eq!(
        err.message,
        format!("Invalid property: '{}...' (200 bytes)", "é".repeat(64))
    );
    // Two units together longer than one shown whole are not shortened
    let err = convert(1.0, "mg{creatinine}/dL", "mmol{creatinine}/L{serum}").unwrap_err();
    assert_eq!(
        err.message,
        "Dimension mismatch in conversion from 'mg{creatinine}/dL' to \
         'mmol{creatinine}/L{serum}': expected L-3, found L-3.M"
    );
}
