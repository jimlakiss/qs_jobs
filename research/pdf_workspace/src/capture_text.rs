//! Plain values shared by recognition, review and export.
/// Remove layout whitespace and invisible PDF/OCR control characters without
/// removing meaningful punctuation, accents, or quoted text.
pub fn plain(text: &str) -> String {
    let visible: String = text
        .chars()
        .filter(|c| {
            !matches!(c,
        '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' |
        '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    visible.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn number(text: &str, max_len: usize) -> Option<u32> {
    if text.is_empty() || text.len() > max_len || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}
fn day_number(text: &str) -> Option<u32> {
    for suffix in ["st", "nd", "rd", "th"] {
        if let Some(digits) = text.strip_suffix(suffix) {
            let day = number(digits, 2)?;
            let expected = if (11..=13).contains(&(day % 100)) {
                "th"
            } else {
                match day % 10 {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                }
            };
            return (suffix == expected).then_some(day);
        }
    }
    number(text, 2)
}
fn month_number(text: &str) -> Option<u32> {
    Some(match text {
        "jan" | "january" => 1,
        "feb" | "february" => 2,
        "mar" | "march" => 3,
        "apr" | "april" => 4,
        "may" => 5,
        "jun" | "june" => 6,
        "jul" | "july" => 7,
        "aug" | "august" => 8,
        "sep" | "sept" | "september" => 9,
        "oct" | "october" => 10,
        "nov" | "november" => 11,
        "dec" | "december" => 12,
        _ => return None,
    })
}
fn date(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut parts: Vec<_> = lower
        .split(|c: char| c.is_whitespace() || matches!(c, '/' | '-' | '.' | ','))
        .filter(|p| !p.is_empty())
        .collect();
    if parts.first().is_some_and(|p| {
        matches!(
            *p,
            "mon"
                | "monday"
                | "tue"
                | "tues"
                | "tuesday"
                | "wed"
                | "wednesday"
                | "thu"
                | "thur"
                | "thurs"
                | "thursday"
                | "fri"
                | "friday"
                | "sat"
                | "saturday"
                | "sun"
                | "sunday"
        )
    }) {
        parts.remove(0);
    }
    if parts.len() != 3 {
        return None;
    }
    // Four-digit years are required: do not guess a century from OCR.
    let (day, month, year) = if let Some(month) = month_number(parts[1]) {
        if parts[0].len() == 4 && number(parts[0], 4).is_some() {
            (day_number(parts[2])?, month, number(parts[0], 4)?)
        } else {
            if parts[2].len() != 4 {
                return None;
            }
            (day_number(parts[0])?, month, number(parts[2], 4)?)
        }
    } else if let Some(month) = month_number(parts[0]) {
        if parts[2].len() != 4 {
            return None;
        }
        (day_number(parts[1])?, month, number(parts[2], 4)?)
    } else if parts[0].len() == 4 {
        (
            number(parts[2], 2)?,
            number(parts[1], 2)?,
            number(parts[0], 4)?,
        )
    } else {
        if parts[2].len() != 4 {
            return None;
        }
        let first = number(parts[0], 2)?;
        let second = number(parts[1], 2)?;
        // Australian day-first order; accept month-first when unambiguous.
        let (day, month) = if first <= 12 && second > 12 {
            (second, first)
        } else {
            (first, second)
        };
        (day, month, number(parts[2], 4)?)
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    if year == 0 || day == 0 || day > days {
        return None;
    }
    Some(format!("{day:02}/{month:02}/{year:04}"))
}
pub fn normalize(id: &str, text: &str) -> String {
    let text = plain(text);
    if id == "date" {
        date(&text).unwrap_or(text)
    } else {
        text
    }
}
pub fn date_warning(text: &str) -> Option<String> {
    let text = plain(text);
    (!text.is_empty() && date(&text).is_none())
        .then(|| "Date is ambiguous or invalid; review the visible drawing (DD/MM/YYYY).".into())
}
/// Multiple text runs can be obsolete labels hidden by a scanned revision.
/// Verify the visible pixels rather than exporting both values or guessing.
pub fn verify_visible(id: &str, text: &str) -> bool {
    text.lines().filter(|line| !line.trim().is_empty()).count() > 1
        || (id == "date" && date_warning(text).is_some())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_values_keep_content_without_pdf_layout() {
        assert_eq!(
            plain("  Café, \"North\"\r\n\tLevel\u{a0}2\u{200b}\0 "),
            "Café, \"North\" Level 2"
        );
        assert_eq!(normalize("date", " 4 / 7 / 2026\r\n"), "04/07/2026");
        assert_eq!(normalize("date", "14-04-2026"), "14/04/2026");
    }
    #[test]
    fn common_date_formats_normalise_to_australian_dates() {
        for text in [
            "21 July 2026",
            "21 JUL 2026",
            "21-Jul-2026",
            "21st July 2026",
            "July 21, 2026",
            "Jul. 21st, 2026",
            "Tuesday, 21 July 2026",
            "2026-07-21",
            "2026/07/21",
            "2026 July 21",
            "07/21/2026",
            "21.07.2026",
            "21 07 2026",
        ] {
            assert_eq!(normalize("date", text), "21/07/2026", "{text}");
            assert!(date_warning(text).is_none(), "{text}");
            assert!(!verify_visible("date", text), "{text}");
        }
        assert_eq!(normalize("date", "3/4/2026"), "03/04/2026");
        assert_eq!(normalize("date", "1st Sept. 2026"), "01/09/2026");
        assert_eq!(normalize("date", "February 29, 2024"), "29/02/2024");
        for text in [
            "21 July 26",
            "July 32, 2026",
            "29 February 2026",
            "31 April 2026",
            "11st July 2026",
            "21 July 2026 / 22 July 2026",
        ] {
            assert!(date_warning(text).is_some(), "{text}");
        }
    }
    #[test]
    fn conflicting_and_invalid_dates_are_not_silently_chosen() {
        for text in [
            "27/03/2026\n14/04/2026",
            "31/04/2026",
            "29/02/2025",
            "14/O4/2026",
        ] {
            assert!(date_warning(text).is_some());
            assert!(verify_visible("date", text));
        }
        assert!(date_warning("29/02/2024").is_none());
        assert!(date_warning("").is_none());
        assert!(verify_visible(
            "description",
            "GROUND FLOOR PLAN\nBASEMENT FLOOR PLAN"
        ));
        assert!(!verify_visible("description", "BASEMENT FLOOR PLAN"));
    }
}
