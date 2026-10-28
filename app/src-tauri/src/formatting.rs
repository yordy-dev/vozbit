use regex::{Captures, Regex, RegexBuilder};

fn replace_ci(text: &str, pattern: &str, replacement: &str) -> String {
    RegexBuilder::new(pattern)
        .case_insensitive(true)
        .unicode(true)
        .build()
        .expect("patrón interno válido")
        .replace_all(text, replacement)
        .into_owned()
}

fn number_value(word: &str) -> Option<i64> {
    match word {
        "cero" => Some(0),
        "un" | "uno" | "una" => Some(1),
        "dos" => Some(2),
        "tres" => Some(3),
        "cuatro" => Some(4),
        "cinco" => Some(5),
        "seis" => Some(6),
        "siete" => Some(7),
        "ocho" => Some(8),
        "nueve" => Some(9),
        "diez" => Some(10),
        "once" => Some(11),
        "doce" => Some(12),
        "trece" => Some(13),
        "catorce" => Some(14),
        "quince" => Some(15),
        "dieciséis" | "dieciseis" => Some(16),
        "diecisiete" => Some(17),
        "dieciocho" => Some(18),
        "diecinueve" => Some(19),
        "veinte" => Some(20),
        "veintiuno" | "veintiún" | "veintiun" | "veintiuna" => Some(21),
        "veintidós" | "veintidos" => Some(22),
        "veintitrés" | "veintitres" => Some(23),
        "veinticuatro" => Some(24),
        "veinticinco" => Some(25),
        "veintiséis" | "veintiseis" => Some(26),
        "veintisiete" => Some(27),
        "veintiocho" => Some(28),
        "veintinueve" => Some(29),
        "treinta" => Some(30),
        "cuarenta" => Some(40),
        "cincuenta" => Some(50),
        "sesenta" => Some(60),
        "setenta" => Some(70),
        "ochenta" => Some(80),
        "noventa" => Some(90),
        "cien" | "ciento" => Some(100),
        "doscientos" | "doscientas" => Some(200),
        "trescientos" | "trescientas" => Some(300),
        "cuatrocientos" | "cuatrocientas" => Some(400),
        "quinientos" | "quinientas" => Some(500),
        "seiscientos" | "seiscientas" => Some(600),
        "setecientos" | "setecientas" => Some(700),
        "ochocientos" | "ochocientas" => Some(800),
        "novecientos" | "novecientas" => Some(900),
        _ => None,
    }
}

fn parse_spanish_number(phrase: &str) -> Option<i64> {
    let words = phrase
        .split(|character: char| character.is_whitespace() || character == '-')
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect::<Vec<_>>();
    if words.is_empty()
        || (words.len() == 1 && matches!(words[0].as_str(), "un" | "una"))
    {
        return None;
    }
    if words.last().is_some_and(|w| w == "y") { return None; }
    let mut previous = None;
    for (i, word) in words.iter().enumerate() {
        if word == "y" {
            if i == 0 || i + 1 >= words.len() { return None; }
            let tens = number_value(&words[i-1]).unwrap_or(0);
            let unit = number_value(&words[i+1]).unwrap_or(0);
            if tens < 30 || tens > 90 || tens % 10 != 0 || !(1..=9).contains(&unit) { return None; }
            continue;
        }
        if let Some(n) = number_value(word) {
            if let Some(p) = previous {
                if !(p >= 100 && n < 100 || p >= 30 && p <= 90 && i > 0 && words[i-1] == "y") { return None; }
            }
            previous = Some(n);
        } else { previous = None; }
    }
    let mut total = 0_i64;
    let mut group = 0_i64;
    let mut saw_number = false;
    for word in words {
        match word.as_str() {
            "y" => continue,
            "mil" => {
                group = group.max(1);
                total = total.checked_add(group.checked_mul(1_000)?)?;
                group = 0;
                saw_number = true;
            }
            "millón" | "millon" | "millones" => {
                total = total.checked_add(group)?.max(1).checked_mul(1_000_000)?;
                group = 0;
                saw_number = true;
            }
            other => {
                group = group.checked_add(number_value(other)?)?;
                saw_number = true;
            }
        }
    }
    saw_number.then_some(total + group)
}

fn normalize_spanish_numbers(text: &str) -> String {
    let words = "cero|un|uno|una|dos|tres|cuatro|cinco|seis|siete|ocho|nueve|diez|once|doce|trece|catorce|quince|dieciséis|dieciseis|diecisiete|dieciocho|diecinueve|veinte|veintiuno|veintiún|veintiun|veintiuna|veintidós|veintidos|veintitrés|veintitres|veinticuatro|veinticinco|veintiséis|veintiseis|veintisiete|veintiocho|veintinueve|treinta|cuarenta|cincuenta|sesenta|setenta|ochenta|noventa|cien|ciento|doscientos|doscientas|trescientos|trescientas|cuatrocientos|cuatrocientas|quinientos|quinientas|seiscientos|seiscientas|setecientos|setecientas|ochocientos|ochocientas|novecientos|novecientas|mil|millón|millon|millones";
    let pattern = format!(r"(?iu)\b(?:{words})(?:[\s-]+(?:y|{words}))*\b");
    Regex::new(&pattern)
        .expect("patrón interno válido")
        .replace_all(text, |captures: &Captures<'_>| {
            parse_spanish_number(&captures[0])
                .map(|number| number.to_string())
                .unwrap_or_else(|| captures[0].split_whitespace().map(|word| {
                    if matches!(word.to_lowercase().as_str(), "un" | "una" | "y") { word.to_string() }
                    else { number_value(&word.to_lowercase()).map(|n|n.to_string()).unwrap_or_else(||word.to_string()) }
                }).collect::<Vec<_>>().join(" "))
        })
        .into_owned()
}

fn capitalize_sentences(text: &str) -> String {
    let mut capitalize = true;
    let mut result = String::with_capacity(text.len());
    let mut characters=text.chars().peekable();
    while let Some(character) = characters.next() {
        let math_variable = matches!(character, 'x' | 'y' | 'z')
            && characters.clone().find(|next| !next.is_whitespace()).is_some_and(|next| {
                matches!(next, '⁰' | '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹' | '+' | '-' | '−' | '=' | '×' | '÷')
            });
        if capitalize && character.is_alphabetic() && !math_variable {
            for upper in character.to_uppercase() {
                result.push(upper);
            }
            capitalize = false;
        } else {
            result.push(character);
            if math_variable { capitalize = false; }
        }
        if character.is_numeric() { capitalize=false; }
        if character == '\n' || (matches!(character, '.' | '!' | '?') && characters.peek().is_none_or(|c|c.is_whitespace())) {
            capitalize = true;
        }
    }
    result
}

fn normalize_spacing(text: &str) -> String {
    let mut value = replace_ci(text, r"[ \t]+([,.;:!?%])", "$1");
    // Keep decimal separators, versions, URLs and identifiers intact.
    value = replace_ci(&value, r"([¿¡\(\[\{])\s+", "$1");
    value = replace_ci(&value, r"\s+([\)\]\}])", "$1");
    value = replace_ci(&value, r"[ \t]{2,}", " ");
    value = replace_ci(&value, r" *\n *", "\n");
    value.trim().to_string()
}

fn normalize_ruc(text: &str) -> String {
    // An incomplete identifier must not turn a proper name into a RUC.
    RegexBuilder::new(r"(?P<label>\b(?:RUC|Rook)\b)(?P<link>\s+(?:viene a ser|es(?: el)?|n[uú]mero(?: de)?))?\s+(?P<groups>\d{1,3}(?:(?:\s*,\s*|\s+y\s+|\s+)\d{1,3})+)")
        .case_insensitive(true).build().expect("patrón interno válido")
        .replace_all(text, |captures: &Captures<'_>| {
            let digits: String = captures["groups"].chars().filter(char::is_ascii_digit).collect();
            if digits.len() == 11 { format!("RUC{} {}", captures.name("link").map_or("", |part| part.as_str()), digits) }
            else { captures[0].to_string() }
        }).into_owned()
}

fn normalize_equations(text: &str) -> String {
    let term = r"(?:\d+(?:\s*[xyz])?(?:[⁰¹²³⁴⁵⁶⁷⁸⁹]+)?|[xyz](?:[⁰¹²³⁴⁵⁶⁷⁸⁹]+)?)";
    // A sentence break between two signed terms is likely automatic ASR punctuation
    // only when an equality follows. Explicitly spoken "signo punto" is still words
    // at this stage, so it is never removed here.
    let interrupted = Regex::new(&format!(r"(?iu)\b(?P<head>{term}\s*(?:m[aá]s|menos|\+|−|-)\s*{term})\s*\.\s*(?P<tail>(?:m[aá]s|menos|\+|−|-)\s*{term}\s*,?\s*(?:(?:es\s+)?igual(?:\s+a)?|=)\s*{term})\b"))
        .expect("patrón interno válido");
    let text = interrupted.replace_all(text, "${head} ${tail}");
    let pattern = format!(r"(?iu)\b(?P<left>{term}(?:\s*,?\s*(?:m[aá]s|menos|\+|−|-)\s*{term})+)(?:\s*,?\s*(?:(?:es\s+)?igual(?:\s+a)?|=)\s*(?P<right>{term}))?");
    let equation = Regex::new(&pattern).expect("patrón interno válido");
    let operator = Regex::new(r"(?iu)\s*,?\s*(m[aá]s|menos|\+|−|-)\s*").expect("patrón interno válido");
    let variable = Regex::new(r"(?iu)(\d)\s+([xyz])\b").expect("patrón interno válido");
    equation.replace_all(&text, |captures: &Captures<'_>| {
        let left = operator.replace_all(&captures["left"], |part: &Captures<'_>| {
            if part[1].eq_ignore_ascii_case("menos") || &part[1] == "−" || &part[1] == "-" { " - ".to_string() }
            else { " + ".to_string() }
        });
        let left = variable.replace_all(&left, "$1$2");
        match captures.name("right") {
            Some(right) => format!("{} = {}", left, variable.replace_all(right.as_str(), "$1$2")),
            None => left.into_owned(),
        }
    }).into_owned()
}

fn subscript_digits(value: &str) -> String {
    value.chars().map(|digit| match digit {
        '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
        '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
        other => other,
    }).collect()
}

fn superscript_digits(value: &str) -> String {
    value.chars().map(|digit| match digit {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
        '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
        other => other,
    }).collect()
}

fn render_logarithm(base: &str, argument: &str) -> String {
    if base.chars().all(|digit| digit.is_ascii_digit()) {
        return format!("log{}({argument})", subscript_digits(base));
    }
    let root = Regex::new(r"(?iu)^(?:ra[ií]z(?:\s+cuadrada)?\s+de\s*|√\s*)(\d+)$")
        .expect("patrón interno válido");
    if let Some(captures) = root.captures(base) {
        return format!("log_(√{})({argument})", &captures[1]);
    }
    format!("log_({base})({argument})")
}

fn normalize_logarithms(text: &str) -> String {
    let raised_number = r"\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]*";
    let base = format!(r"(?:{raised_number}|(?:ra[ií]z(?:\s+cuadrada)?\s+de\s*|√\s*)\d+)");
    let already_written = Regex::new(&format!(r"(?iu)\blog_(?P<base>\d+)\s*\(\s*(?P<argument>{raised_number})\s*\)"))
        .expect("patrón interno válido");
    let value = already_written.replace_all(text, |captures: &Captures<'_>| {
        render_logarithm(&captures["base"], &captures["argument"])
    });
    // "logaritmo de 4 en base 32" has the opposite order from
    // "logaritmo en base 4 de 32". Do not infer a base without that word.
    let reverse = Regex::new(&format!(r"(?iu)\b(?:logaritmo|aridmo)\s+de\s+(?P<argument>{raised_number})\s+(?:en|de)\s+base\s+(?:(?:de|a)\s+)?(?P<base>{base})"))
        .expect("patrón interno válido");
    let value = reverse.replace_all(&value, |captures: &Captures<'_>| {
        render_logarithm(&captures["base"], &captures["argument"])
    });
    let standard = Regex::new(&format!(r"(?iu)\b(?:logaritmo|aridmo)\s+(?:(?:en|de)\s+)?base\s+(?:de\s+)?(?P<base>{base})\s*,?\s*(?:(?:de|a|en)\s+(?:(?:la|el|las)\s+)?|\^\s*(?:(?:la|las)\s+)?)(?P<argument>{raised_number})"))
        .expect("patrón interno válido");
    let value = standard.replace_all(&value, |captures: &Captures<'_>| {
        render_logarithm(&captures["base"], &captures["argument"])
    }).into_owned();
    // An unspecified base stays unspecified. Do not consume a phrase whose
    // following "en base" was not recognized by the explicit-base patterns.
    let bare = Regex::new(&format!(r"(?iu)\b(?:logaritmo|aridmo)\s+de\s+(?P<argument>{raised_number})"))
        .expect("patrón interno válido");
    bare.replace_all(&value, |captures: &Captures<'_>| {
        let following = value[captures.get(0).expect("captura completa").end()..].trim_start().to_lowercase();
        if following.starts_with("en base") || following.starts_with("de base") {
            captures[0].to_string()
        } else {
            format!("log({})", &captures["argument"])
        }
    }).into_owned()
}

fn normalize_powers(text: &str) -> String {
    let power = Regex::new(r"(?iu)\b(?P<base>\d+|[xyz])\s*(?:elevad[oa]\s+a\s+(?:(?:la|el)\s+)?|\^\s*(?:(?:la|el)\s+)?)(?P<exponent>\d+)\b")
        .expect("patrón interno válido");
    let value = power.replace_all(text, |captures: &Captures<'_>| {
        format!("{}{}", &captures["base"], superscript_digits(&captures["exponent"]))
    });
    let square_or_cube = Regex::new(r"(?iu)\b(?P<base>\d+|[xyz])\s+al\s+(?P<power>cuadrado|cubo)\b")
        .expect("patrón interno válido");
    square_or_cube.replace_all(&value, |captures: &Captures<'_>| {
        format!("{}{}", &captures["base"], if captures["power"].eq_ignore_ascii_case("cubo") { "³" } else { "²" })
    }).into_owned()
}

fn normalize_numeric_comparisons(text: &str) -> String {
    let pattern = Regex::new(r"(?iu)\b(?P<left>\d+)\s+(?:es\s+)?(?P<relation>mayor\s+o\s+igual\s+que|menor\s+o\s+igual\s+que|mayor\s+que|menor\s+que|igual(?:\s+a)?)\s+(?P<right>\d+)\b")
        .expect("patrón interno válido");
    pattern.replace_all(text, |captures: &Captures<'_>| {
        let relation = captures["relation"].to_lowercase();
        let symbol = if relation.starts_with("mayor o igual") { "≥" }
            else if relation.starts_with("menor o igual") { "≤" }
            else if relation.starts_with("mayor") { ">" }
            else if relation.starts_with("menor") { "<" }
            else { "=" };
        format!("{} {symbol} {}", &captures["left"], &captures["right"])
    }).into_owned()
}

fn normalize_fractions_and_roots(text: &str) -> String {
    let root_fraction = Regex::new(r"(?iu)\b(?P<numerator>\d+)\s+sobre\s+(?:(?:la|el)\s+)?(?:ra[ií]z(?:\s+cuadrada)?\s+de\s*|√\s*)(?P<radicand>\d+)\b")
        .expect("patrón interno válido");
    let value = root_fraction.replace_all(text, "${numerator}/√${radicand}");
    let simple_fraction = Regex::new(r"(?iu)\b(?P<numerator>\d+)\s+sobre\s+(?P<denominator>\d+)\b")
        .expect("patrón interno válido");
    let value = simple_fraction.replace_all(&value, "${numerator}/${denominator}");
    let root = Regex::new(r"(?iu)\bra[ií]z(?:\s+cuadrada)?\s+de\s+(?P<radicand>\d+)\b")
        .expect("patrón interno válido");
    let value = root.replace_all(&value, "√${radicand}");
    replace_ci(&value, r"√\s+(\d+)\b", "√$1")
}

fn normalize_math_equality(text: &str) -> String {
    let atom = r"(?:log(?:[₀₁₂₃₄₅₆₇₈₉]+|_\((?:√\d+|\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]+)\))?\(\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]*\)|\d+/√?\d+|√\d+|(?:\d+\s*)?[xyz][⁰¹²³⁴⁵⁶⁷⁸⁹]*|\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]+)";
    let pattern = Regex::new(&format!(r"(?iu)(?P<expression>{atom})\s*,?\s*(?:(?:es\s+)?igual(?:\s+a)?|=)\s*(?:(?:la|el)\s+)?(?P<result>{atom}|\d+)"))
        .expect("patrón interno válido");
    let first = pattern.replace_all(text, "${expression} = ${result}");
    pattern.replace_all(&first, "${expression} = ${result}").into_owned()
}

fn is_standalone_math_expression(text: &str) -> bool {
    let logarithm = Regex::new(r"log(?:[₀₁₂₃₄₅₆₇₈₉]+|_\((?:√\d+|\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]+)\))?\(\d+[⁰¹²³⁴⁵⁶⁷⁸⁹]*\)")
        .expect("patrón interno válido");
    let has_math_notation = logarithm.is_match(text) || text.chars().any(|character| matches!(character,
        '⁰' | '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹'
        | '√' | '+' | '-' | '−' | '=' | '×' | '÷' | '/' | '%' | '<' | '>' | '≥' | '≤'));
    let without_logarithms = logarithm.replace_all(text, "");
    has_math_notation && without_logarithms.chars().all(|character| {
        character.is_ascii_digit() || character.is_whitespace()
            || matches!(character, 'x' | 'y' | 'z' | 'X' | 'Y' | 'Z'
                | '⁰' | '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹'
                | '√' | '+' | '-' | '−' | '=' | '×' | '÷' | '/' | '%' | '<' | '>' | '≥' | '≤'
                | '(' | ')')
    })
}

fn format_explicit_code(text: &str) -> Option<String> {
    let marker = Regex::new(r"(?iu)\b(?:abrir llaves|cerrar llaves|abrir corchetes|cerrar corchetes|guion bajo|barra invertida|acento grave|sangr[ií]a|palabra clave if|doble igual|(?:unif|un if)\s+abrir par[eé]ntesis)\b").expect("patrón interno válido");
    if !marker.is_match(text) { return None; }
    let mut value = text.to_string();
    for (spoken, symbol) in [
        ("palabra clave if", "if"),
        ("nueva línea", "\n"), ("nueva linea", "\n"),
        ("abrir llaves", "{"), ("cerrar llaves", "}"),
        ("abrir corchetes", "["), ("cerrar corchetes", "]"),
        ("abrir paréntesis", "("), ("cerrar paréntesis", ")"),
        ("abrir parentesis", "("), ("cerrar parentesis", ")"),
        ("comilla doble", "\""), ("comilla simple", "'"), ("acento grave", "`"),
        ("signo punto y coma", ";"), ("signo dos puntos", ":"),
        ("signo punto", "."), ("signo coma", ","), ("signo asterisco", "*"),
        ("signo porcentaje", "%"),
        ("signo igual", "="), ("signo más", "+"), ("signo menos", "-"),
        ("doble igual", "=="),
        ("barra invertida", "\\"), ("signo barra", "/"), ("guion bajo", "_"),
        ("menor que", "<"), ("mayor que", ">"),
    ] { value = replace_ci(&value, &format!(r"\b{}\b", regex::escape(spoken)), symbol); }
    value = replace_ci(&value, r"\b(?:unif|un if)\b", "if");
    value = replace_ci(&value, r"\bsangr[ií]a\b\s*", "    ");
    value = Regex::new(r"(?iu)\bn[uú]mero\s+([\p{L}]+(?:\s+y\s+[\p{L}]+)?)")
        .expect("patrón interno válido")
        .replace_all(&value, |captures: &Captures<'_>| parse_spanish_number(&captures[1])
            .map(|number| number.to_string()).unwrap_or_else(|| captures[0].to_string()))
        .into_owned();
    value = replace_ci(&value, r"\(\s+", "(");
    value = replace_ci(&value, r"\s+\)", ")");
    value = replace_ci(&value, r"\[\s+", "[");
    value = replace_ci(&value, r"\s+\]", "]");
    Some(value)
}

pub(crate) fn format_transcript(
    text: &str,
    language: &str,
    text_correction: bool,
    format_numbers: bool,
    math_symbols: bool,
) -> String {
    let mut value = text.trim().to_string();
    if value.is_empty() { return value; }
    if let Some(code) = format_explicit_code(&value) { return code; }
    if language == "en" {
        if text_correction && !value.ends_with(['.', '!', '?']) {
            value.push('.');
        }
        return if text_correction { capitalize_sentences(&normalize_spacing(&value)) } else { value };
    }

    if text_correction {
        let punctuation = [
            (r"\b(?:nueva línea|nueva linea|salto de línea|salto de linea)\b", "\n"),
            (r"\bpunto y coma\b", ";"),
            (r"\bdos puntos\b", ":"),
            (r"\babrir interrogaci[oó]n\b", "¿"),
            (r"\b(?:cerrar interrogaci[oó]n|signo de interrogaci[oó]n)\b", "?"),
            (r"\babrir exclamaci[oó]n\b", "¡"),
            (r"\b(?:cerrar exclamaci[oó]n|signo de exclamaci[oó]n)\b", "!"),
            (r"\babrir par[eé]ntesis\b", "("),
            (r"\bcerrar par[eé]ntesis\b", ")"),
        ];
        for (pattern, replacement) in punctuation {
            value = replace_ci(&value, pattern, replacement);
        }
    }

    if math_symbols { value = replace_ci(&value, r"\bpor ciento\b", "%"); }
    if format_numbers {
        value = normalize_spanish_numbers(&value);
        value = normalize_ruc(&value);
    }

    if math_symbols {
        let explicit = [
            (r"\bra[ií]z cuadrada de\b", "√"),
            (r"\bmultiplicado por\b", "×"),
            (r"\bdividido (?:entre|por)\b", "÷"),
            (r"\bsigno m[aá]s\b", "+"),
            (r"\bsigno menos\b", "−"),
            (r"\bsigno igual\b", "="),
            (r"\bpor ciento\b", "%"),
        ];
        for (pattern, replacement) in explicit {
            value = replace_ci(&value, pattern, replacement);
        }
        value = normalize_powers(&value);
        value = normalize_logarithms(&value);
        value = normalize_equations(&value);
        value = normalize_fractions_and_roots(&value);
        value = normalize_math_equality(&value);
        value = normalize_numeric_comparisons(&value);
    }

    // Spoken punctuation is applied after math parsing, so an explicitly
    // dictated full stop cannot be mistaken for an ASR-inserted interruption.
    if text_correction {
        value = replace_ci(&value, r"\bsigno coma\b", ",");
        value = replace_ci(&value, r"\bsigno punto\b", ".");
    }

    value = normalize_spacing(&value);
    if text_correction {
        value = capitalize_sentences(&value);
        value = replace_ci(&value, r"\bLog([₀₁₂₃₄₅₆₇₈₉])", "log$1");
        value = replace_ci(&value, r"\bLog_\(", "log_(");
        value = replace_ci(&value, r"\bLog\(", "log(");
        if !is_standalone_math_expression(&value)
            && !matches!(value.as_str(), "=" | "+" | "−" | "×" | "÷" | "%" | "<" | ">" | "≥" | "≤")
            && !value.ends_with(['.', '!', '?', ':', ';']) {
            value.push('.');
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_punctuation_is_rendered() {
        assert_eq!(
            format_transcript("hola signo coma cómo estás signo de interrogación", "es", true, true, true),
            "Hola, cómo estás?"
        );
    }

    #[test]
    fn spanish_numbers_are_grouped() {
        assert_eq!(
            format_transcript("tengo dos mil trescientos cuarenta y cinco archivos", "es", true, true, true),
            "Tengo 2345 archivos."
        );
    }

    #[test]
    fn common_math_is_converted_only_between_numbers() {
        assert_eq!(
            format_transcript("dos más dos es igual a cuatro", "es", true, true, true),
            "2 + 2 = 4"
        );
        assert_eq!(
            format_transcript("quiero más tiempo", "es", true, true, true),
            "Quiero más tiempo."
        );
    }

    #[test]
    fn articles_are_not_forced_to_digits() {
        assert_eq!(
            format_transcript("un documento", "es", true, true, true),
            "Un documento."
        );
    }

    #[test]
    fn grouped_ruc_is_joined_without_joining_unrelated_lists() {
        assert_eq!(format_transcript("El Rook viene a ser 10, 40, 90, 27, 27 y 6", "es", true, true, true), "El RUC viene a ser 10409027276.");
        assert_eq!(format_transcript("Compré 10, 40 y 90 unidades", "es", true, true, true), "Compré 10, 40 y 90 unidades.");
        assert_eq!(format_transcript("El Rook viene a ser 10, 40 y 90", "es", true, true, true), "El Rook viene a ser 10, 40 y 90.");
    }

    #[test]
    fn dictated_equation_uses_operators_and_variables() {
        assert_eq!(format_transcript("La ecuación es 13x más 4, menos 12x, igual a 0", "es", true, true, true), "La ecuación es 13x + 4 - 12x = 0.");
    }

    #[test]
    fn logarithm_with_spoken_base_has_readable_notation() {
        assert_eq!(format_transcript("logaritmo en base 4 de 16", "es", true, true, true), "log₄(16)");
        assert_eq!(format_transcript("aridmo en base 4, ^ las 16", "es", true, true, true), "log₄(16)");
        assert_eq!(format_transcript("logaritmo de base cuatro a la dieciséis igual a dieciséis", "es", true, true, true), "log₄(16) = 16");
        assert_eq!(format_transcript("logaritmo de base 4 a la 16, igual a la 16", "es", true, true, true), "log₄(16) = 16");
        assert_eq!(format_transcript("logaritmo en base dos de treinta y dos", "es", true, true, true), "log₂(32)");
        assert_eq!(format_transcript("logaritmo de cuatro en base treinta y dos", "es", true, true, true), "log₃₂(4)");
        assert_eq!(format_transcript("logaritmo de uno en base de raíz de cuatro", "es", true, true, true), "log_(√4)(1)");
        assert_eq!(format_transcript("logaritmo de dos", "es", true, true, true), "log(2)");
        assert_eq!(format_transcript("logaritmo de dos en base a dos elevado a treinta y dos", "es", true, true, true), "log_(2³²)(2)");
        assert_eq!(format_transcript("log_2(32)", "es", true, true, true), "log₂(32)");
    }

    #[test]
    fn mixed_formulas_keep_dictated_order_without_claiming_they_are_true() {
        let spoken = "Bien, ahora vamos a practicar las ecuaciones. Vendría a ser 2 ^ la 8 igual log_2(32) = 0, 3x + 2, 3x + 4 = 0. Potencia de 2 ^ 8.";
        let expected = "Bien, ahora vamos a practicar las ecuaciones. Vendría a ser 2⁸ = log₂(32) = 0, 3x + 2, 3x + 4 = 0. Potencia de 2⁸.";
        assert_eq!(format_transcript(spoken, "es", true, true, true), expected);
        assert_eq!(format_transcript("Escribo logaritmo de dos y después 3x más cuatro igual a cero", "es", true, true, true),
            "Escribo log(2) y después 3x + 4 = 0.");
        assert_eq!(format_transcript("esta cosa es igual a esta", "es", true, true, true), "Esta cosa es igual a esta.");
    }

    #[test]
    fn powers_and_equation_interruption_preserve_meaning() {
        assert_eq!(format_transcript("dos elevado a ocho", "es", true, true, true), "2⁸");
        assert_eq!(format_transcript("cuatro ^ la treinta y dos", "es", true, true, true), "4³²");
        assert_eq!(format_transcript("x al cuadrado más uno igual cero", "es", true, true, true), "x² + 1 = 0");
        assert_eq!(format_transcript("12x menos 0. Más 4x igual 0", "es", true, true, true), "12x - 0 + 4x = 0");
        assert_eq!(format_transcript("12x menos 0 signo punto más 4x igual 0", "es", true, true, true), "12x - 0. Más 4x = 0.");
        assert_eq!(format_transcript("logaritmo de 4 ^ la 32", "es", true, true, true), "log(4³²)");
    }

    #[test]
    fn everyday_equality_is_not_changed_to_an_operator() {
        assert_eq!(format_transcript("esta cosa es igual a esta", "es", true, true, true), "Esta cosa es igual a esta.");
        assert_eq!(format_transcript("A es igual a B", "es", true, true, true), "A es igual a B.");
        assert_eq!(format_transcript("este resultado es igual al anterior", "es", true, true, true), "Este resultado es igual al anterior.");
        assert_eq!(format_transcript("cinco es mayor o igual que cuatro", "es", true, true, true), "5 ≥ 4");
        assert_eq!(format_transcript("dos es igual a dos", "es", true, true, true), "2 = 2");
        assert_eq!(format_transcript("signo igual", "es", true, true, true), "=");
        assert_eq!(format_transcript("doble igual", "es", true, true, true), "==");
        assert_eq!(format_transcript("esta cosa es igual a esta", "es", true, true, false), "Esta cosa es igual a esta.");
        assert_eq!(format_transcript("dos más dos es igual a cuatro", "es", true, true, false), "2 más 2 es igual a 4.");
    }

    #[test]
    fn fractions_and_roots_are_not_applied_to_ordinary_prose() {
        assert_eq!(format_transcript("uno sobre raíz de cuatro", "es", true, true, true), "1/√4");
        assert_eq!(format_transcript("tres sobre cuatro", "es", true, true, true), "3/4");
        assert_eq!(format_transcript("hablamos sobre cuatro temas", "es", true, true, true), "Hablamos sobre 4 temas.");
    }

    #[test]
    fn never_guess_a_different_coefficient_or_incomplete_ruc() {
        assert_eq!(format_transcript("la ecuación es 3x más 4 menos 13x igual a 0", "es", true, true, true), "La ecuación es 3x + 4 - 13x = 0.");
        assert_eq!(format_transcript("RUC es 40, 90, 27, 27, 0", "es", true, true, true), "RUC es 40, 90, 27, 27, 0.");
    }

    #[test]
    fn explicit_code_commands_do_not_require_a_profile() {
        assert_eq!(format_transcript("miVariable signo igual 4 abrir llaves cerrar llaves", "es", true, true, true), "miVariable = 4 { }");
        assert_eq!(format_transcript("sangría miVariable signo igual número dos", "es", true, true, true), "    miVariable = 2");
        assert_eq!(format_transcript("palabra clave if abrir paréntesis x mayor que 0 cerrar paréntesis abrir llaves", "es", true, true, true), "if (x > 0) {");
        assert_eq!(format_transcript("unif abrir paréntesis x cerrar paréntesis", "es", true, true, true), "if (x)");
    }

    #[test]
    fn reported_transcript_is_corrected_without_changing_its_opening() {
        let input = "Hola hola, hola campeón hola. ¿Qué hagas una especie de anotación? El Rook viene a ser 10, 40, 90, 27, 27 y 6. Y la parte de la ecuación es 13x más 4, menos 12x, igual a 0. La otra vendría a ser lo de aridmo en base 4, ^ las 16.";
        let expected = "Hola hola, hola campeón hola. ¿Qué hagas una especie de anotación? El RUC viene a ser 10409027276. Y la parte de la ecuación es 13x + 4 - 12x = 0. La otra vendría a ser lo de log₄(16).";
        assert_eq!(format_transcript(input, "es", true, true, true), expected);
    }
}
