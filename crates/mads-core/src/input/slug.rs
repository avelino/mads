use unicode_normalization::UnicodeNormalization;

/// ASCII slug: NFD, drop combining marks, lowercase, runs of non `[a-z0-9]` become one `-`.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in s
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
    {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_accents_and_lowercases() {
        assert_eq!(slugify("Angélica Zapata"), "angelica-zapata");
    }

    #[test]
    fn collapses_runs_and_trims_edges() {
        assert_eq!(
            slugify("  D.V. Catena -- Cabernet!  "),
            "d-v-catena-cabernet"
        );
    }

    #[test]
    fn handles_nfd_input() {
        assert_eq!(
            slugify("Esporão Reserva"),
            slugify("Espora\u{303}o Reserva")
        );
    }

    #[test]
    fn empty_when_no_alphanumerics() {
        assert_eq!(slugify("!!!"), "");
    }
}
