pub(crate) const ORD_SECURE_CM_IPC: &str = "_ord_secure_cm";
pub(crate) const ORD_SECURE_CM_ARGS: &[&str] = &["--cm", "--ord-secure-ui"];

pub(crate) fn is_ord_secure_cm(args: &[String], available: bool) -> bool {
    available
        && args.len() == 2
        && args[0] == ORD_SECURE_CM_ARGS[0]
        && args[1] == ORD_SECURE_CM_ARGS[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_secure_cm_args_select_dedicated_mode() {
        let args = vec!["--cm".to_owned(), "--ord-secure-ui".to_owned()];
        assert!(is_ord_secure_cm(&args, true));
    }

    #[test]
    fn legacy_or_unavailable_cm_never_selects_dedicated_mode() {
        let ordinary = vec!["--cm".to_owned()];
        let extra = vec![
            "--cm".to_owned(),
            "--ord-secure-ui".to_owned(),
            "--other".to_owned(),
        ];
        let secure = vec!["--cm".to_owned(), "--ord-secure-ui".to_owned()];
        assert!(!is_ord_secure_cm(&ordinary, true));
        assert!(!is_ord_secure_cm(&extra, true));
        assert!(!is_ord_secure_cm(&secure, false));
    }
}
