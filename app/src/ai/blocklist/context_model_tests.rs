use super::*;

#[test]
fn push_keeping_most_recent_keeps_items_under_the_limit() {
    let mut items = vec![1, 2];

    push_keeping_most_recent(&mut items, 3, MAX_AUTO_ATTACHED_USER_BLOCKS);

    assert_eq!(items, vec![1, 2, 3]);
}

#[test]
fn push_keeping_most_recent_drops_oldest_over_the_limit() {
    let mut items = Vec::new();

    for item in 1..=6 {
        push_keeping_most_recent(&mut items, item, MAX_AUTO_ATTACHED_USER_BLOCKS);
    }

    assert_eq!(items, vec![2, 3, 4, 5, 6]);
}

#[test]
fn push_keeping_most_recent_with_zero_limit_keeps_nothing() {
    let mut items = vec![1];

    push_keeping_most_recent(&mut items, 2, 0);

    assert!(items.is_empty());
}
