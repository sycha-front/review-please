use crate::models::EventKind;

pub fn should_mark_done(event_kind: &str, actor_is_me: bool) -> bool {
    if !actor_is_me {
        return false;
    }
    matches!(
        event_kind,
        value if value == EventKind::Commented.as_str()
            || value == EventKind::ReviewCommented.as_str()
            || value == EventKind::Approved.as_str()
            || value == EventKind::ChangesRequested.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::should_mark_done;

    #[test]
    fn marks_done_only_for_my_review_actions() {
        assert!(should_mark_done("approved", true));
        assert!(should_mark_done("commented", true));
        assert!(!should_mark_done("approved", false));
        assert!(!should_mark_done("unknown", true));
    }
}
