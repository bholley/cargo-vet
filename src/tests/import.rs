use super::*;

// Helper function for imports tests. Performs a vet and updates imports based
// on it, returning a diff of the two.
fn get_imports_file_changes(metadata: &Metadata, store: &Store, force_updates: bool) -> String {
    // Run the resolver before calling `get_imports_file` to compute the new
    // imports file.
    let report = crate::resolver::resolve(metadata, None, store, ResolveDepth::Shallow);
    let new_imports =
        store.get_updated_imports_file(&report.graph, &report.conclusion, force_updates);

    // Format the old and new files as TOML, and write out a diff using `similar`.
    let old_imports = crate::serialization::to_formatted_toml(&store.imports)
        .unwrap()
        .to_string();
    let new_imports = crate::serialization::to_formatted_toml(new_imports)
        .unwrap()
        .to_string();

    generate_diff(&old_imports, &new_imports)
}

// Test cases:

#[test]
fn new_peer_import() {
    // (Pass) We import all audits for our third-party packages from a brand-new
    // peer even though we are already fully audited. This won't force an import
    // from existing peers.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [
            (
                "third-party2".to_owned(),
                vec![delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY)],
            ),
            (
                "unused-package".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
        ]
        .into_iter()
        .collect(),
    };

    let old_other_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    let new_other_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![delta_audit(ver(200), ver(300), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    imports
        .audits
        .insert(OTHER_FOREIGN.to_owned(), old_other_foreign_audits);

    config.imports.insert(
        OTHER_FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: OTHER_FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![
            (FOREIGN.to_owned(), new_foreign_audits),
            (OTHER_FOREIGN.to_owned(), new_other_foreign_audits),
        ],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_skip_import() {
    // (Pass) If we've previously imported from a peer, we don't import
    // audits for a package unless it's useful.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [
            (
                "third-party2".to_owned(),
                vec![delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY)],
            ),
            (
                "unused-package".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
        ]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_remove_unused() {
    // (Pass) We'll remove unused audits when unlocked, even if our peer hasn't
    // changed.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [
            (
                "third-party2".to_owned(),
                vec![delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY)],
            ),
            (
                "unused-package".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
        ]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = old_foreign_audits.clone();

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_import_delta_audit() {
    // (Pass) If a new delta audit from a peer is useful, we'll import it and
    // all other audits for that crate, including from other peers.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, mut audits, mut imports) = builtin_files_full_audited(&metadata);

    audits.audits.remove("third-party2");

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![full_audit(ver(9), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [
            // A new audit for third-party2 should fix our audit, so we should
            // import all of these.
            (
                "third-party2".to_owned(),
                vec![
                    full_audit(ver(9), SAFE_TO_DEPLOY),
                    delta_audit(ver(9), ver(DEFAULT_VER), SAFE_TO_DEPLOY),
                    delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY),
                ],
            ),
            // This audit won't change things for us, so we won't import it to
            // avoid churn.
            (
                "third-party1".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
        ]
        .into_iter()
        .collect(),
    };

    let old_other_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    let new_other_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        // We'll also import unrelated audits from other sources.
        audits: [(
            "third-party2".to_owned(),
            vec![delta_audit(ver(200), ver(300), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    imports
        .audits
        .insert(OTHER_FOREIGN.to_owned(), old_other_foreign_audits);

    config.imports.insert(
        OTHER_FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: OTHER_FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![
            (FOREIGN.to_owned(), new_foreign_audits),
            (OTHER_FOREIGN.to_owned(), new_other_foreign_audits),
        ],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_import_custom_criteria() {
    // (Pass) We'll immediately import criteria changes wen unlocked, even if
    // our peer hasn't changed or we aren't mapping them locally. This doesn't
    // force an import of other crates.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);
    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: [("fuzzed".to_string(), criteria("fuzzed"))]
            .into_iter()
            .collect(),
        audits: [(
            "third-party2".to_owned(),
            vec![
                full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY),
                delta_audit(ver(DEFAULT_VER), ver(11), SAFE_TO_DEPLOY),
            ],
        )]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);

    insta::assert_snapshot!(output);
}

#[test]
fn new_audit_for_unused_criteria_basic() {
    // (Pass) If a peer adds an audit for an unused criteria, we shouldn't
    // vendor in the changes unnecessarily.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);
    let old_foreign_audits = AuditsFile {
        criteria: [("fuzzed".to_string(), criteria("fuzzed"))]
            .into_iter()
            .collect(),
        audits: [(
            "third-party2".to_owned(),
            vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let mut new_foreign_audits = old_foreign_audits.clone();
    new_foreign_audits
        .audits
        .get_mut("third-party2")
        .unwrap()
        .push(full_audit(ver(DEFAULT_VER), "fuzzed"));

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);

    insta::assert_snapshot!(output);
}

#[test]
fn new_audit_for_unused_criteria_transitive() {
    // (Pass) If a peer adds an audit for an unused criteria of a transitive
    // dependency, we shouldn't vendor in the changes unnecessarily.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);
    let old_foreign_audits = AuditsFile {
        criteria: [("fuzzed".to_string(), criteria("fuzzed"))]
            .into_iter()
            .collect(),
        audits: [(
            "third-party1".to_owned(),
            vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let mut new_foreign_audits = old_foreign_audits.clone();
    new_foreign_audits
        .audits
        .get_mut("third-party1")
        .unwrap()
        .push(full_audit(ver(DEFAULT_VER), "fuzzed"));
    new_foreign_audits.audits.insert(
        "transitive-third-party1".to_owned(),
        vec![full_audit(ver(DEFAULT_VER), "fuzzed")],
    );

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);

    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_revoked_audit() {
    // (Pass) If a previously-imported audit is removed, we should also remove
    // it locally, even if we don't use it.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn existing_peer_add_violation() {
    // (Pass) If a peer adds a violation for any version of a crate we use, we
    // should immediately import it. We won't immediately import other audits
    // added for that crate, however.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_full_audited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![
                delta_audit(ver(100), ver(200), SAFE_TO_DEPLOY),
                delta_audit(ver(200), ver(300), SAFE_TO_DEPLOY),
                violation(VersionReq::parse("99.*").unwrap(), SAFE_TO_DEPLOY),
            ],
        )]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn peer_audits_exemption_no_minimize() {
    // (Pass) We don't import audits for a package which would replace an
    // exemption unless we're regenerating exemptions.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_inited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

#[test]
fn peer_audits_exemption_minimize() {
    // (Pass) We do import audits for a package which would replace an exemption
    // when we're regenerating exemptions.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, audits, mut imports) = builtin_files_inited(&metadata);

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: SortedMap::new(),
    };

    let new_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [(
            "third-party2".to_owned(),
            vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
        )]
        .into_iter()
        .collect(),
    };

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            ..Default::default()
        },
    );

    let mut store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    // Capture the old imports before minimizing exemptions
    let old = store.mock_commit();

    crate::resolver::regenerate_exemptions(&mock_cfg(&metadata), &mut store, true, false).unwrap();

    // Capture after minimizing exemptions, and generate a diff.
    let new = store.mock_commit();

    let output = diff_store_commits(&old, &new);
    insta::assert_snapshot!(output);
}

#[test]
fn peer_audits_import_exclusion() {
    // (Pass) Exclusions in the config should make a crate's audits and
    // violations appear to be revoked upstream, but audits for other crates
    // shouldn't be impacted.

    let _enter = TEST_RUNTIME.enter();
    let mock = MockMetadata::simple();

    let metadata = mock.metadata();
    let (mut config, mut audits, mut imports) = builtin_files_full_audited(&metadata);

    audits.audits.remove("transitive-third-party1");

    let old_foreign_audits = AuditsFile {
        criteria: SortedMap::new(),
        audits: [
            (
                "third-party2".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
            (
                "third-party1".to_owned(),
                vec![violation("*".parse().unwrap(), SAFE_TO_DEPLOY)],
            ),
            (
                "transitive-third-party1".to_owned(),
                vec![full_audit(ver(DEFAULT_VER), SAFE_TO_DEPLOY)],
            ),
        ]
        .into_iter()
        .collect(),
    };

    let new_foreign_audits = old_foreign_audits.clone();

    imports
        .audits
        .insert(FOREIGN.to_owned(), old_foreign_audits);

    config.imports.insert(
        FOREIGN.to_owned(),
        crate::format::RemoteImport {
            url: FOREIGN_URL.to_owned(),
            exclude: vec!["third-party1".to_owned(), "third-party2".to_owned()],
            ..Default::default()
        },
    );

    let store = Store::mock_online(
        config,
        audits,
        imports,
        vec![(FOREIGN.to_owned(), new_foreign_audits)],
        true,
    )
    .unwrap();

    let imported = store
        .imported_audits()
        .get(FOREIGN)
        .expect("The remote should be present in `imported_audits`");

    assert!(
        !imported.audits.contains_key("third-party1"),
        "The `third-party1` crate should be completely missing from `imported_audits`"
    );
    assert!(
        !imported.audits.contains_key("third-party2"),
        "The `third-party2` crate should be completely missing from `imported_audits`"
    );
    assert!(
        imported.audits.contains_key("transitive-third-party1"),
        "The `transitive-third-party1` crate should still be present in `imported_audits`"
    );

    let output = get_imports_file_changes(&metadata, &store, false);
    insta::assert_snapshot!(output);
}

// Other tests worth adding:
//
// - used edges with dependency-criteria should cause criteria to be imported
