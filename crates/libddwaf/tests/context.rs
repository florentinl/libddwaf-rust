#![cfg(not(miri))]
#![warn(
    clippy::correctness,
    clippy::pedantic,
    clippy::perf,
    clippy::style,
    clippy::suspicious
)]

use std::sync::LazyLock;
use std::{sync::Arc, time::Duration};

use libddwaf::object::{LibddwafDefaultAllocator, WafOwnedDefaultAllocator};
use libddwaf::{
    object::{WafArray, WafMap, WafObject},
    waf_array, waf_map, Builder, Config, RunResult, RunnableContext,
};

static ARACHNI_RULE: LazyLock<WafMap> = LazyLock::new(|| {
    waf_map! {
        ("version", "2.1"),
        ("rules", waf_array![
            waf_map!{
                ("id", "arachni_rule"),
                ("name", "Block with default action"),
                ("tags", waf_map!{ ("category", "attack_attempt"), ("type", "security_scanner") }),
                ("conditions", waf_array![
                    waf_map!{
                        ("operator", "match_regex"),
                        ("parameters", waf_map!{
                            ("inputs", waf_array![
                                waf_map!{
                                    ("address", "server.request.headers.no_cookies"),
                                    ("key_path", waf_array!["user-agent"]),
                                },
                                waf_map!{
                                    ("address", "server.request.body"),
                                },
                            ]),
                            ("regex", "Arachni"),
                        }),
                    },
                ]),
                ("on_match", waf_array!["block"])
            },
        ]),
    }
});

static SPLIT_ADDRESS_RULE: LazyLock<WafMap> = LazyLock::new(|| {
    waf_map! {
        ("version", "2.1"),
        ("rules", waf_array![
            waf_map!{
                ("id", "split_address_rule"),
                ("name", "Block when inputs are spread across batches"),
                ("tags", waf_map!{ ("category", "attack_attempt"), ("type", "security_scanner") }),
                ("conditions", waf_array![
                    waf_map!{
                        ("operator", "match_regex"),
                        ("parameters", waf_map!{
                            ("inputs", waf_array![
                                waf_map!{
                                    ("address", "test.first"),
                                },
                            ]),
                            ("regex", "first-value"),
                        }),
                    },
                    waf_map!{
                        ("operator", "match_regex"),
                        ("parameters", waf_map!{
                            ("inputs", waf_array![
                                waf_map!{
                                    ("address", "test.second"),
                                },
                            ]),
                            ("regex", "second-value"),
                        }),
                    },
                ]),
                ("on_match", waf_array!["block"])
            },
        ]),
    }
});

#[test]
fn basic_run_rule_with_match() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    let mut diagnostics = WafOwnedDefaultAllocator::<WafMap>::default();
    assert!(builder.add_or_update_config(
        "rules",
        LazyLock::force(&ARACHNI_RULE),
        Some(&mut diagnostics)
    ));

    assert!(diagnostics.is_valid());
    let loaded_rule_name = diagnostics
        .get_str("rules")
        .unwrap()
        .as_type::<WafMap>()
        .unwrap()
        .get_str("loaded")
        .unwrap()
        .as_type::<WafArray>()
        .unwrap()[0]
        .to_str()
        .unwrap();
    assert_eq!(loaded_rule_name, "arachni_rule");

    let waf = builder.build().unwrap();
    let mut ctx = waf.new_context();

    let mut header = WafMap::new(1);
    header[0] = ("user-agent", "Arachni").into();
    let mut data = WafMap::new(1);
    data[0] = ("server.request.headers.no_cookies", header).into();

    let res = ctx.run(data, Duration::from_secs(1));

    match res {
        Ok(RunResult::Match(result)) => {
            assert!(!result.timeout());
            assert!(result.keep());
            assert!(result.duration() > Duration::default());
            assert_eq!(result.evaluated(), 1);

            let events = result.events().expect("Expected some events");
            assert_eq!(events.len(), 1);
            let first_event: &WafMap = events[0].as_type().unwrap();
            let rule_first_event: &WafMap = first_event.get_str("rule").unwrap().as_type().unwrap();
            assert_eq!(
                rule_first_event.get_str("id").unwrap().to_str().unwrap(),
                "arachni_rule"
            );

            let actions = result.actions().expect("Expected some actions");
            assert_eq!(actions.len(), 1);
            assert!(actions.get_bstr(b"block_request").is_some(),);
        }
        _ => {
            panic!("Unexpected result: {res:?}");
        }
    }
}

#[test]
fn basic_run_rule_with_no_match() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    let mut diagnostics = WafOwnedDefaultAllocator::<WafMap>::default();
    assert!(builder.add_or_update_config(
        "rules",
        LazyLock::force(&ARACHNI_RULE),
        Some(&mut diagnostics)
    ));

    assert!(diagnostics.is_valid());
    let loaded_rule_name = diagnostics
        .get_str("rules")
        .unwrap()
        .as_type::<WafMap>()
        .unwrap()
        .get_str("loaded")
        .unwrap()
        .as_type::<WafArray>()
        .unwrap()[0]
        .to_str()
        .unwrap();
    assert_eq!(loaded_rule_name, "arachni_rule");

    let waf = builder.build().unwrap();
    let mut ctx = waf.new_context();

    let mut header = WafMap::new(1);
    header[0] = ("user-agent", "JDatabaseDriverMysqli").into();
    let mut data = WafMap::new(1);
    data[0] = ("server.request.headers.no_cookies", header).into();

    let res = ctx.run(data, Duration::from_secs(1));

    match res {
        Ok(RunResult::NoMatch(result)) => {
            assert!(!result.timeout());
            assert!(!result.keep());
            assert!(result.duration() > Duration::default());
            assert_eq!(result.evaluated(), 1);

            if let Some(events) = result.events() {
                assert!(events.is_empty());
            }
            if let Some(actions) = result.actions() {
                assert!(actions.is_empty());
            }
            if let Some(attributes) = result.attributes() {
                assert!(attributes.is_empty());
            }
        }
        _ => {
            panic!("Unexpected result: {res:?}");
        }
    }
}

#[test]
fn run_owned_default_allocator_rule_with_match() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", LazyLock::force(&ARACHNI_RULE), None));
    let waf = builder.build().unwrap();
    let mut ctx = waf.new_context();

    let mut data = WafOwnedDefaultAllocator::<WafObject>::default();
    data.set_map::<LibddwafDefaultAllocator>(1).unwrap();
    let headers = data
        .insert_key::<LibddwafDefaultAllocator>("server.request.headers.no_cookies")
        .unwrap();
    headers.set_map::<LibddwafDefaultAllocator>(1).unwrap();
    headers
        .insert_key::<LibddwafDefaultAllocator>("user-agent")
        .unwrap()
        .set_string::<LibddwafDefaultAllocator>("Arachni")
        .unwrap();

    let res = ctx.run_owned(data, Duration::from_secs(1));

    match res {
        Ok(RunResult::Match(result)) => {
            assert!(!result.timeout());
            assert!(result.keep());
            assert_eq!(result.evaluated(), 1);
            assert_eq!(result.events().expect("Expected some events").len(), 1);
        }
        _ => {
            panic!("Unexpected result: {res:?}");
        }
    }
}

#[test]
fn run_batches_context_matches_across_batches() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", LazyLock::force(&SPLIT_ADDRESS_RULE), None));
    let waf = builder.build().unwrap();
    let mut ctx = waf.new_context();

    let data = waf_array![
        waf_map!(("test.first", "first-value")),
        waf_map!(("test.second", "second-value")),
    ];

    let res = ctx.run_batches(data, Duration::from_secs(1));

    assert_split_address_match(res);
}

#[test]
fn run_batches_subcontext_matches_across_batches() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", LazyLock::force(&SPLIT_ADDRESS_RULE), None));
    let waf = builder.build().unwrap();
    let ctx = waf.new_context();
    let mut subctx = ctx.new_subcontext().unwrap();

    let data = waf_array![
        waf_map!(("test.first", "first-value")),
        waf_map!(("test.second", "second-value")),
    ];

    let res = subctx.run_batches(data, Duration::from_secs(1));

    assert_split_address_match(res);
}

fn assert_split_address_match(res: Result<RunResult, libddwaf::RunError>) {
    match res {
        Ok(RunResult::Match(result)) => {
            assert!(!result.timeout());
            assert!(result.keep());
            assert_eq!(result.evaluated(), 2);

            let events = result.events().expect("Expected some events");
            assert_eq!(events.len(), 1);
            let first_event: &WafMap = events[0].as_type().unwrap();
            let rule_first_event: &WafMap = first_event.get_str("rule").unwrap().as_type().unwrap();
            assert_eq!(
                rule_first_event.get_str("id").unwrap().to_str().unwrap(),
                "split_address_rule"
            );

            let actions = result.actions().expect("Expected some actions");
            assert_eq!(actions.len(), 1);
            assert!(actions.get_bstr(b"block_request").is_some());
        }
        _ => {
            panic!("Unexpected result: {res:?}");
        }
    }
}

#[test]
fn run_rule_threaded() {
    let mut builder = Builder::new(Some(&Config::default())).expect("Failed to create builder");
    assert!(builder.add_or_update_config("rules", LazyLock::force(&ARACHNI_RULE), None));
    let waf = Arc::new(builder.build().unwrap());

    let mut header = WafMap::new(1);
    header[0] = ("user-agent", "Arachni").into();
    let mut data = WafMap::new(1);
    data[0] = (
        "server.request.headers.no_cookies",
        Into::<WafObject>::into(header),
    )
        .into();

    let threads: Vec<_> = (0..2)
        .map(|_| {
            let waf = waf.clone();
            let data = data.clone();
            std::thread::spawn(move || {
                let ctx = Arc::new(waf.new_context());

                (0..2)
                    .map(|_| {
                        let ctx = ctx.clone();
                        let data = data.clone();
                        std::thread::spawn(move || {
                            let mut subctx = (*ctx).new_subcontext().unwrap();

                            let res = subctx.run(data, Duration::from_secs(1));

                            if !matches!(res, Ok(RunResult::Match(_))) {
                                eprintln!("Unexpected result: {res:?}");
                            }
                            assert!(matches!(res, Ok(RunResult::Match(_))));
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .for_each(|t| t.join().unwrap());
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn test_run_error_display() {
    use libddwaf::RunError;

    assert_eq!(
        format!("{}", RunError::InternalError),
        "The WAF encountered an internal error"
    );
    assert_eq!(
        format!("{}", RunError::InvalidObject),
        "The WAF encountered an invalid object"
    );
    assert_eq!(
        format!("{}", RunError::InvalidArgument),
        "The WAF encountered an invalid argument"
    );
}

#[test]
fn test_run_output_debug() {
    let mut builder = Builder::new(Some(&Config::default())).expect("builder should be created");
    let mut diagnostics = WafOwnedDefaultAllocator::<WafMap>::default();
    assert!(builder.add_or_update_config("test", &*ARACHNI_RULE, Some(&mut diagnostics)));
    let waf = builder.build().expect("waf should be created");

    let mut ctx = waf.new_context();
    let data = waf_map! {
        ("server.request.headers.no_cookies", waf_map!{
            ("user-agent", "Arachni"),
        }),
    };

    match ctx.run(data, Duration::from_secs(1)) {
        Ok(RunResult::Match(output)) => {
            // Test that Debug formatting works
            let debug_str = format!("{output:?}");
            assert!(debug_str.contains("RunOutput"));
            assert!(debug_str.contains("timeout"));
            assert!(debug_str.contains("keep"));
            assert!(debug_str.contains("duration"));
            assert!(debug_str.contains("evaluated"));
        }
        other => panic!("Expected match result, got: {other:?}"),
    }
}
