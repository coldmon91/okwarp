use super::*;

#[test]
fn test_repo_name() {
    assert_eq!(repo_name(Channel::Dev), "swarf-dev");
    assert_eq!(repo_name(Channel::Stable), "swarf");
}
