use trail_config::{Config, Mapping, Value};

#[test]
fn ambiguous_separator_boundaries_are_marked() {
    let config = Config::load_yaml("'a:':\n  b: 1\na:\n  ':b': 2\n", "::").unwrap();
    assert_eq!(config.outline(), concat!(
        "a:::b: <number>  # not addressable\n",
        "a:::b: <number>\n",
    ));
    assert_eq!(config.get_int("a:::b"), Some(2));
    // The subtree remains readable through its addressable parent.
    assert_eq!(config.get_as::<Value>("a:").unwrap()["b"].as_i64(), Some(1));
}

#[test]
fn equal_values_do_not_hide_ambiguous_paths() {
    let config = Config::load_yaml("'a:':\n  b:\n    c: 1\na:\n  ':b':\n    c: 1\n", "::").unwrap();
    assert_eq!(config.outline(), concat!(
        "a:::b::c: <number>  # not addressable\n",
        "a:::b::c: <number>\n",
    ));
}

#[test]
fn generated_outline_paths_retrieve_their_original_leaves() {
    // Enumerate keys containing separator prefixes, suffixes, whole separators,
    // overlapping separators, and backslashes. Unique leaf IDs catch paths that
    // resolve successfully but navigate to a different leaf.
    for separator in ["/", "::", "->", "aba", "aaa", "→", "→→", "éé"] {
        let first = separator.chars().next().unwrap().to_string();
        let last = separator.chars().last().unwrap().to_string();
        let mut candidates = vec![
            "x".to_owned(), first.clone(), last.clone(),
            format!("x{first}"), format!("{last}x"), separator.to_owned(),
            format!("{separator}{first}"), format!("{first}{separator}"),
            format!("{separator}{separator}"), "\\".to_owned(),
            format!("{first}\\"), format!("\\{last}"),
        ];
        for (boundary, _) in separator.char_indices().skip(1) {
            let (prefix, suffix) = separator.split_at(boundary);
            candidates.extend([
                prefix.to_owned(), suffix.to_owned(),
                format!("x{prefix}"), format!("{suffix}x"),
            ]);
        }
        let mut keys = Vec::new();
        for key in candidates {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        let mut root = Mapping::new();
        let mut id = 0_i64;
        for parent in &keys {
            let mut children = Mapping::new();
            for child in &keys {
                children.insert(Value::String(child.clone()), Value::Number(id.into()));
                id += 1;
            }
            root.insert(Value::String(parent.clone()), Value::Mapping(children));
        }
        let yaml = yaml_serde::to_string(&root).unwrap();
        let config = Config::load_yaml(&yaml, separator).unwrap();
        let outline = config.outline();
        assert_eq!(outline.lines().count(), id as usize);
        let mut unmarked = 0;
        for (expected, line) in outline.lines().enumerate() {
            let (path, _) = line.rsplit_once(": <number>").unwrap();
            if line.ends_with("# not addressable") {
                assert_ne!(config.get_int(path), Some(expected as i64), "false marker: {separator:?} {line}");
            } else {
                unmarked += 1;
                assert_eq!(config.get_int(path), Some(expected as i64), "wrong leaf: {separator:?} {line}");
            }
        }
        assert!(unmarked > 0);
        if separator.chars().count() == 1 {
            assert_eq!(unmarked, id);
        }
    }
}

#[test]
fn ambiguous_paths_are_marked_for_tagged_and_container_leaves() {
    for leaf in ["[]", "{}", "!Label 1"] {
        let yaml = format!("'a:': !Parent\n  b: {leaf}\na:\n  ':b': {leaf}\n");
        let config = Config::load_yaml(&yaml, "::").unwrap();
        let outline = config.outline();
        let lines: Vec<_> = outline.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("# not addressable"), "{outline}");
        assert!(!lines[1].ends_with("# not addressable"), "{outline}");
    }
}
