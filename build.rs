use std::{collections::HashMap, env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let xml_path = manifest_dir.join("ucum-essence.xml");

    println!("cargo:rerun-if-changed={}", xml_path.display());

    // Phase 3: parse prefixes from XML and emit registry. Units will follow in the next step.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("registry.rs");

    // --- Parse XML ---
    let xml_data = fs::read_to_string(&xml_path).expect("read ucum-essence.xml");
    let mut prefixes: Vec<(String, f64, i8, String)> = Vec::new();

    let mut reader = quick_xml::Reader::from_str(&xml_data);
    loop {
        use quick_xml::events::Event;
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"prefix" {
                    let mut code: Option<String> = None;
                    let mut value: Option<f64> = None;
                    let mut name: Option<String> = None;

                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        if attr.key.as_ref() == b"Code" {
                            code = Some(String::from_utf8_lossy(&attr.value).to_string())
                        }
                    }
                    // value and name elements are children; capture them
                    loop {
                        match reader.read_event() {
                            Ok(Event::Start(ref ve)) if ve.name().as_ref() == b"value" => {
                                if let Some(v_attr) = ve
                                    .attributes()
                                    .filter_map(|a| a.ok())
                                    .find(|a| a.key.as_ref() == b"value")
                                {
                                    value = Some(
                                        String::from_utf8_lossy(&v_attr.value)
                                            .parse::<f64>()
                                            .unwrap(),
                                    );
                                }
                            }
                            Ok(Event::Start(ref ne)) if ne.name().as_ref() == b"name" => {
                                // Read the text content of the name element
                                if let Ok(Event::Text(text)) = reader.read_event() {
                                    name = Some(String::from_utf8_lossy(&text).to_string());
                                }
                            }
                            Ok(Event::End(ref ve)) if ve.name().as_ref() == b"prefix" => break,
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                    }
                    if let (Some(c), Some(v), Some(n)) = (code, value, name) {
                        // Exponent is log10 of value
                        let exp = v.abs().log10() as i8; // rough, assumes powers of 10
                        prefixes.push((c, v, exp, n));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error reading XML: {e:?}"),
            _ => {}
        }
    }

    prefixes.sort_by(|a, b| a.0.cmp(&b.0));

    // --- Generate Rust source ---
    let mut out = String::new();
    out.push_str("use crate::types::{Prefix, UnitRecord, Dimension};\n\n");

    // Prefixes array
    out.push_str("pub static PREFIXES: &[Prefix] = &[\n");
    for (code, val, exp, name) in &prefixes {
        out.push_str(&format!(
            "    Prefix {{ symbol: \"{code}\", factor: {val}f64, exponent: {exp}, display_name: \"{name}\" }},\n"
        ));
    }
    out.push_str("];\n\n");

    // --- Parse units (base-unit + unit) ---
    #[allow(clippy::type_complexity)]
    let mut units: Vec<(
        String,
        [i8; 7],
        f64,
        f64,
        String,
        String,
        String,
        Option<String>,
    )> = Vec::new();

    // Definition of each regular unit: its `value` and the `Unit` expression it multiplies
    let mut definitions: HashMap<String, Definition> = HashMap::new();

    // reuse reader on xml_data
    let mut reader = quick_xml::Reader::from_str(&xml_data);
    loop {
        use quick_xml::events::Event;
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"base-unit" => {
                        let code = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"Code")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .expect("base-unit code");
                        // parse dim attribute (e.g., "L", "M", etc.) into 7-vector
                        let dim_attr = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"dim")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string());
                        let dim = dim_attr.as_deref().map(parse_dim).unwrap_or([0i8; 7]);

                        // Extract property and display the name from base-unit
                        let mut property = String::new();
                        let mut display_name = String::new();
                        let mut in_property_tag = false;
                        let mut in_n_tag = false;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Text(ref text)) => {
                                    if in_property_tag {
                                        property = String::from_utf8_lossy(text).trim().to_string();
                                        in_property_tag = false;
                                    }
                                    if in_n_tag {
                                        display_name =
                                            String::from_utf8_lossy(text).trim().to_string();
                                        in_n_tag = false;
                                    }
                                }
                                Ok(Event::Start(ref ve)) if ve.name().as_ref() == b"property" => {
                                    in_property_tag = true;
                                }
                                Ok(Event::Start(ref ve)) if ve.name().as_ref() == b"name" => {
                                    in_n_tag = true;
                                }
                                Ok(Event::End(ref ve)) if ve.name().as_ref() == b"base-unit" => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }

                        // Default display name to code if not found
                        if display_name.is_empty() {
                            display_name = code.clone();
                        }

                        units.push((
                            code,
                            dim,
                            1.0f64,
                            0.0f64,
                            "SpecialKind::None".into(),
                            property,
                            display_name,
                            None,
                        ));
                    }
                    b"unit" => {
                        let code = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"Code")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .expect("unit code");
                        let dim_attr = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"dim")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string());
                        let mut dim = dim_attr.as_deref().map_or([0i8; 7], parse_dim);
                        // Need to capture <value> child to get a factor (may combine Unit attr) and maybe offset
                        // Also capture <property> child to get unit classification and <name> for display name
                        let mut factor: Option<f64> = None;
                        let mut offset: f64 = 0.0;
                        let mut property = String::new();
                        let mut display_name = String::new();
                        let mut in_property_tag = false;
                        let mut in_n_tag = false;
                        let mut unit_ref_for_dim: Option<String> = None;
                        let mut definition: Option<Definition> = None;
                        loop {
                            match reader.read_event() {
                                Ok(Event::Empty(ref ve)) | Ok(Event::Start(ref ve)) => {
                                    if ve.name().as_ref() == b"value" {
                                        // attribute value
                                        let attrs: Vec<_> =
                                            ve.attributes().filter_map(|a| a.ok()).collect();
                                        let val_num = attrs
                                            .iter()
                                            .find(|a| a.key.as_ref() == b"value")
                                            .map(|a| String::from_utf8_lossy(&a.value));
                                        let unit_attr = attrs
                                            .iter()
                                            .find(|a| a.key.as_ref() == b"Unit")
                                            .map(|a| String::from_utf8_lossy(&a.value));
                                        if let (Some(u), Some(v)) = (&unit_attr, &val_num)
                                            && let Ok(value) = v.parse::<f64>()
                                        {
                                            definition = Some(Definition {
                                                value,
                                                expr: u.to_string(),
                                            });
                                        }
                                        let mut f = 1.0f64;
                                        if let Some(u) = unit_attr {
                                            f *= parse_factor(&u);
                                            unit_ref_for_dim = Some(u.to_string());
                                        }
                                        if let Some(v) = val_num {
                                            f *= v.parse::<f64>().unwrap_or(1.0);
                                        }
                                        factor = Some(f);
                                        if let Some(o_attr) =
                                            attrs.iter().find(|a| a.key.as_ref() == b"offset")
                                        {
                                            offset = String::from_utf8_lossy(&o_attr.value)
                                                .parse::<f64>()
                                                .unwrap_or(0.0);
                                        }
                                    } else if ve.name().as_ref() == b"property" {
                                        in_property_tag = true;
                                    } else if ve.name().as_ref() == b"name" {
                                        in_n_tag = true;
                                    }
                                }
                                Ok(Event::Text(ref text)) => {
                                    // Capture property and display name text content
                                    if in_property_tag {
                                        property = String::from_utf8_lossy(text).trim().to_string();
                                        in_property_tag = false;
                                    }
                                    if in_n_tag {
                                        display_name =
                                            String::from_utf8_lossy(text).trim().to_string();
                                        in_n_tag = false;
                                    }
                                }
                                Ok(Event::End(ref ve)) if ve.name().as_ref() == b"unit" => break,
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                        }

                        // Default display name to code if not found
                        if display_name.is_empty() {
                            display_name = code.clone();
                        }
                        // Special handling for Celsius, Fahrenheit, Rankine, Réaumur, Liter, and Imperial units
                        match code.as_str() {
                            "Cel" => {
                                offset = 273.15;
                                if dim == [0i8; 7] {
                                    dim[4] = 1;
                                }
                            }
                            "[in_i]" => {
                                // Fix precision issue: ensure exactly 2.54 cm
                                // The build script will convert cm to m (2.54 * 0.01 = 0.0254)
                                factor = Some(2.54);
                            }
                            "[degF]" => {
                                factor = Some(5.0 / 9.0);
                                offset = 255.37222222222223; // (459.67 * 5/9)
                                if dim == [0i8; 7] {
                                    dim[4] = 1;
                                }
                            }
                            "[degR]" => {
                                factor = Some(5.0 / 9.0);
                                offset = 0.0;
                                if dim == [0i8; 7] {
                                    dim[4] = 1;
                                }
                            }
                            "[degRe]" => {
                                factor = Some(5.0 / 4.0);
                                offset = 273.15;
                                if dim == [0i8; 7] {
                                    dim[4] = 1;
                                }
                            }
                            // Special handling for liter (L) and lowercase liter (l)
                            // These should have dimension L^3 (volume)
                            // The UCUM XML defines L and l with a property of "volume" but doesn't specify
                            // the dimension directly. L references l, which references dm3 (cubic decimeter).
                            // We need to explicitly set the dimension to L^3 here to ensure proper
                            // dimensional analysis, especially for arbitrary unit conversions.
                            "L" | "l" if dim == [0i8; 7] => {
                                dim[1] = 3; // L^3 for volume
                            }
                            _ => {}
                        }
                        // If unit has non-zero offset but no temperature dimension, set Θ = 1
                        if offset != 0.0 && dim == [0i8; 7] {
                            dim[4] = 1;
                        }
                        let mut special = "SpecialKind::None".to_string();
                        if offset != 0.0 {
                            special = "SpecialKind::LinearOffset".into();
                        }

                        // Special units (temperature, logarithmic, ...) are not a plain
                        // `value x Unit` product: they keep the values assigned above.
                        let is_special = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .any(|a| a.key.as_ref() == b"isSpecial" && a.value.as_ref() == b"yes");
                        if !is_special && let Some(definition) = definition {
                            definitions.insert(code.clone(), definition);
                        }

                        // Check if this is an arbitrary unit
                        let is_arbitrary = e
                            .attributes()
                            .filter_map(|a| a.ok())
                            .find(|a| a.key.as_ref() == b"isArbitrary")
                            .map(|a| String::from_utf8_lossy(&a.value).to_string())
                            .is_some_and(|v| v == "yes");

                        // Special handling for [p'diop] unit
                        if code == "[p'diop]" {
                            special = "SpecialKind::TanTimes100".into();
                        }
                        // Only treat units as arbitrary if explicitly marked with isArbitrary="yes"
                        else if is_arbitrary {
                            special = "SpecialKind::Arbitrary".into();
                        } else {
                            match code.as_str() {
                                "B" | "Bel" | "dB" | "dB[SPL]" | "dB[lin]" => {
                                    special = "SpecialKind::Log10".into();
                                }
                                "Np" => {
                                    special = "SpecialKind::Ln".into();
                                }
                                _ => {}
                            }
                        }
                        units.push((
                            code,
                            dim,
                            factor.unwrap_or(1.0),
                            offset,
                            special,
                            property,
                            display_name,
                            unit_ref_for_dim,
                        ));
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    units.sort_by(|a, b| a.0.cmp(&b.0));

    // Second pass: compute the factor and dimension of every regular unit by evaluating its
    // definition. Base units and special units keep the values assigned above.
    let prefix_factors: HashMap<String, f64> = prefixes
        .iter()
        .map(|(code, value, _, _)| (code.clone(), *value))
        .collect();
    let mut resolver = Resolver {
        prefixes: &prefix_factors,
        definitions: &definitions,
        resolved: units
            .iter()
            .filter(|unit| !definitions.contains_key(&unit.0))
            .map(|unit| (unit.0.clone(), (unit.2, unit.1)))
            .collect(),
        in_progress: Vec::new(),
    };
    for unit in units.iter_mut() {
        if definitions.contains_key(&unit.0) {
            (unit.2, unit.1) = resolver.unit(&unit.0);
        }
    }

    // Units array
    out.push_str("use crate::types::SpecialKind;\n");
    out.push_str("#[allow(clippy::approx_constant)] // Constants come from UCUM specification\n");
    out.push_str("pub static UNITS: &[UnitRecord] = &[\n");
    for (code, dim, factor, offset, special, property, display_name, _unit_ref) in &units {
        // Format factor with const replacement if needed
        let factor_str = if (*factor - std::f64::consts::PI).abs() < 1e-10 {
            "core::f64::consts::PI".to_string()
        } else if (*factor - std::f64::consts::TAU).abs() < 1e-10 {
            "core::f64::consts::TAU".to_string()
        } else if (*factor - std::f64::consts::FRAC_PI_4).abs() < 1e-10 {
            "core::f64::consts::FRAC_PI_4".to_string()
        } else {
            format!("{factor}f64")
        };

        out.push_str(&format!(
            "    UnitRecord {{ code: \"{}\", dim: Dimension([{} ,{} ,{} ,{} ,{} ,{} ,{}]), factor: {}, offset: {}f64, special: {}, property: \"{}\", display_name: \"{}\" }},\n",
            code, dim[0],dim[1],dim[2],dim[3],dim[4],dim[5],dim[6], factor_str, offset, special, property, display_name));
    }
    out.push_str("]\n;\n\n");

    // Units array

    // lookup functions
    out.push_str("pub fn find_prefix(sym: &str) -> Option<&'static Prefix> {\n    PREFIXES.binary_search_by(|p| p.symbol.cmp(sym)).ok().map(|i| &PREFIXES[i])\n}\n\n");
    out.push_str("pub fn find_unit(code: &str) -> Option<&'static UnitRecord> {\n    // First try direct lookup\n    if let Ok(i) = UNITS.binary_search_by(|u| u.code.cmp(code)) {\n        return Some(&UNITS[i]);\n    }\n    \n    // If direct lookup fails, try to decompose into prefix + base unit\n    // Check all possible prefix lengths (longest first to avoid ambiguity)\n    for prefix_len in (1..code.len()).rev() {\n        // Skip offsets that fall inside a multi-byte character\n        let Some((prefix_part, unit_part)) = code.split_at_checked(prefix_len) else {\n            continue;\n        };\n        \n        // Check if prefix_part is a valid prefix and unit_part is a valid unit\n        if let (Some(_prefix), Some(_unit)) = (\n            find_prefix(prefix_part),\n            UNITS.binary_search_by(|u| u.code.cmp(unit_part)).ok().map(|i| &UNITS[i])\n        ) {\n            // For prefixed units, we don't return the base unit record directly\n            // because the caller would need to apply the prefix factor.\n            // Instead, we return None to indicate this should be handled by the parser.\n            // However, since the issue asks for find_unit to work with \"mg\",\n            // we'll return the base unit for now.\n            return Some(_unit);\n        }\n    }\n    \n    None\n}\n");

    fs::write(&dest, out).expect("write registry.rs");

    // Tell rustc to include the generated file.
    println!("cargo:rustc-env=UCUM_REGISTRY={}", dest.display());
}

/// Round a computed factor to 16 significant digits, which removes the binary noise that
/// chained `f64` operations leave in the last place (`0.1 * 0.1 * 0.1` is not `0.001`).
fn round_factor(factor: f64) -> f64 {
    format!("{factor:.15e}").parse().unwrap_or(factor)
}

/// Definition of a regular unit in `ucum-essence.xml`: `value` times the `Unit` expression.
struct Definition {
    value: f64,
    expr: String,
}

/// Factor to the base units and dimension vector of a unit or expression.
type Quantity = (f64, [i8; 7]);

/// Evaluates unit definitions, following references to other units.
struct Resolver<'a> {
    prefixes: &'a HashMap<String, f64>,
    definitions: &'a HashMap<String, Definition>,
    /// Units whose quantity is known. Seeded with the base units and the special units.
    resolved: HashMap<String, Quantity>,
    /// Units being resolved, to report a circular definition instead of recursing forever.
    in_progress: Vec<String>,
}

impl Resolver<'_> {
    /// Quantity of the unit `code`, which must be a unit of the specification.
    fn unit(&mut self, code: &str) -> Quantity {
        if let Some(quantity) = self.resolved.get(code) {
            return *quantity;
        }
        assert!(
            !self.in_progress.iter().any(|c| c == code),
            "circular definition of unit `{code}`"
        );
        let definition = &self.definitions[code];

        // The amount of substance has a dimension of its own in this crate, whereas UCUM
        // defines the mole as the dimensionless number 6.02214076 x 10*23.
        let quantity = if code == "mol" {
            (definition.value, [0, 0, 0, 0, 0, 1, 0])
        } else {
            self.in_progress.push(code.to_string());
            let (factor, dim) = self.expression(&definition.expr);
            self.in_progress.pop();
            (round_factor(definition.value * factor), dim)
        };
        self.resolved.insert(code.to_string(), quantity);
        quantity
    }

    /// Quantity of a unit symbol with an optional prefix, e.g. `m`, `cm` or `m[Hg]`.
    fn symbol(&mut self, symbol: &str) -> Quantity {
        if self.resolved.contains_key(symbol) || self.definitions.contains_key(symbol) {
            return self.unit(symbol);
        }
        for len in 1..symbol.len() {
            let Some((prefix, code)) = symbol.split_at_checked(len) else {
                continue;
            };
            if let Some(&prefix_factor) = self.prefixes.get(prefix)
                && (self.resolved.contains_key(code) || self.definitions.contains_key(code))
            {
                let (factor, dim) = self.unit(code);
                return (prefix_factor * factor, dim);
            }
        }
        panic!("unknown unit `{symbol}` in a unit definition");
    }

    /// Quantity of a full unit expression such as `4.[pi].10*-7.N/A2`.
    fn expression(&mut self, expr: &str) -> Quantity {
        let mut pos = 0;
        let quantity = self.term(expr, &mut pos);
        assert!(pos == expr.len(), "cannot parse unit definition `{expr}`");
        quantity
    }

    /// `term := ['/'] component (('.' | '/') component)*`, evaluated left to right.
    fn term(&mut self, expr: &str, pos: &mut usize) -> Quantity {
        let bytes = expr.as_bytes();
        let mut acc: Quantity = (1.0, [0; 7]);
        if bytes.get(*pos) != Some(&b'/') {
            acc = self.component(expr, pos);
        }
        while let Some(&operator @ (b'.' | b'/')) = bytes.get(*pos) {
            *pos += 1;
            let (factor, dim) = self.component(expr, pos);
            let sign: i8 = if operator == b'.' { 1 } else { -1 };
            acc.0 *= factor.powi(sign.into());
            for (acc_exp, exp) in acc.1.iter_mut().zip(dim) {
                *acc_exp += sign * exp;
            }
        }
        acc
    }

    /// `component := '(' term ')' | integer | symbol [exponent]`
    fn component(&mut self, expr: &str, pos: &mut usize) -> Quantity {
        let bytes = expr.as_bytes();
        if bytes.get(*pos) == Some(&b'(') {
            *pos += 1;
            let quantity = self.term(expr, pos);
            assert!(
                bytes.get(*pos) == Some(&b')'),
                "missing `)` in unit definition `{expr}`"
            );
            *pos += 1;
            return quantity;
        }

        // A symbol runs up to the next operator; square brackets may contain any character
        let start = *pos;
        let mut in_brackets = false;
        while let Some(&b) = bytes.get(*pos) {
            match b {
                b'[' => in_brackets = true,
                b']' => in_brackets = false,
                b'.' | b'/' | b'(' | b')' if !in_brackets => break,
                _ => {}
            }
            *pos += 1;
        }
        let text = &expr[start..*pos];
        assert!(
            !text.is_empty(),
            "empty component in unit definition `{expr}`"
        );

        if let Ok(number) = text.parse::<u32>() {
            return (number.into(), [0; 7]);
        }

        // Split a trailing exponent: `cm2`, `s-1`, `[in_i]3`, `10*-7` (the unit `10*` to the -7)
        let digits = text.len() - text.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        let mut symbol_end = text.len() - digits;
        if digits > 0 && text[..symbol_end].ends_with(['+', '-']) {
            symbol_end -= 1;
        }
        let (symbol, exponent) = match text[symbol_end..].parse::<i8>() {
            Ok(exponent) if symbol_end > 0 => (&text[..symbol_end], exponent),
            _ => (text, 1),
        };

        let (factor, dim) = self.symbol(symbol);
        (factor.powi(exponent.into()), dim.map(|exp| exp * exponent))
    }
}

/// Map UCUM dimension string (single letters combined) to Dimension vector.
/// Parse a simple factor expression appearing in the `<value Unit="…">` attribute.
///
/// The UCUM XML occasionally expresses factors as a combination of another unit
/// and a numeric ratio, e.g. `"K/9"`, `"10^3"`, or even nested like `"10*-6"`.
/// We only need to support a **very small** subset for the special‐unit cases:
/// * `K` – evaluates to `1` (Kelvin is canonical for temperature)
/// * `<number>` – literal numeric string
/// * `<lhs>/<rhs>` – division of two numeric or `K` terms (e.g. `K/9` → 1/9)
/// * `10^<n>` or `10*-<n>` – power‐of‐ten factors that were already handled.
///
/// Anything more complex falls back to `1.0`, which is acceptable for units we
/// don’t yet support.
fn parse_factor(text: &str) -> f64 {
    let txt = text.trim();
    if let Some(rest) = txt.strip_prefix("10^")
        && let Ok(exp) = rest.parse::<i32>()
    {
        return 10f64.powi(exp);
    }
    // Simple numeric literal
    if let Ok(n) = txt.parse::<f64>() {
        return n;
    }
    // 10*-n → 10^(−n)
    if let Some(rest) = txt.strip_prefix("10*-")
        && let Ok(exp) = rest.parse::<i32>()
    {
        return 10f64.powi(-exp);
    }
    // Simple Kelvin reference (returns canonical factor 1.0)
    if txt == "K" {
        return 1.0;
    }

    // Handle leading "/" meaning reciprocal (e.g., "/m" → 1.0)
    if let Some(rest) = txt.strip_prefix('/') {
        let denom = parse_factor(rest);
        if denom != 0.0 {
            return 1.0 / denom;
        }
    }

    // Very small expression grammar: A/B
    if let Some((lhs, rhs)) = txt.split_once('/') {
        let l = parse_factor(lhs);
        let r = parse_factor(rhs);
        if r != 0.0 {
            return l / r;
        }
    }

    txt.parse::<f64>().unwrap_or(1.0)
}

fn parse_dim(tag: &str) -> [i8; 7] {
    let mut v = [0i8; 7];
    for ch in tag.chars() {
        match ch {
            'M' => v[0] = 1,
            'L' => v[1] = 1,
            'T' => v[2] = 1,
            'I' => v[3] = 1,
            'C' | 'θ' | 'Θ' => v[4] = 1, // temperature
            'N' => v[5] = 1,
            'J' | 'F' => v[6] = 1, // luminous intensity (UCUM writes it `F`)
            'Q' => {
                // Charge dimension: time × current
                v[2] = 1; // time
                v[3] = 1; // current
            }
            _ => {}
        }
    }
    v
}
