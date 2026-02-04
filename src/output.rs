use std::sync::OnceLock;

static QUIET: OnceLock<bool> = OnceLock::new();

pub fn is_quiet() -> bool {
    *QUIET.get_or_init(|| {
        std::env::var("Coderev_QUIET")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}
