#![cfg(not(miri))]

use libddwaf::Builder;

use common::ARACHNI_RULE;

mod common;

#[test]
fn test_known_actions() {
    let mut builder = Builder::new(None).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", std::sync::LazyLock::force(&ARACHNI_RULE), None));
    let waf = builder.build().unwrap();

    let actions = waf.known_actions();
    assert!(!actions.is_empty());
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].to_str(), Ok("block_request"));
}

#[test]
fn test_known_addresses() {
    let mut builder = Builder::new(None).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", std::sync::LazyLock::force(&ARACHNI_RULE), None));
    let waf = builder.build().unwrap();

    let addresses = waf.known_addresses();
    assert!(!addresses.is_empty());
    assert_eq!(addresses.len(), 2);
    let strs: Vec<_> = addresses.iter().map(|a| a.to_str().unwrap()).collect();
    assert!(strs.contains(&"server.request.body"));
    assert!(strs.contains(&"server.request.headers.no_cookies"));
}
