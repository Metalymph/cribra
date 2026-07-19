#![forbid(unsafe_code)]

/// Public project name.
pub const PROJECT_NAME: &str = "Silens Scan";

/// Returns the public name of the scanner engine.
#[must_use]
pub const fn project_name() -> &'static str {
    PROJECT_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_project_name() {
        assert_eq!(project_name(), "Silens Scan");
    }
}
