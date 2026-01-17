use bbcode_parser::{ast_to_html, parse_bbcode_to_ast, BbCodeError, BbCodeOptions, Node};

#[test]
fn test_basic_parse() {
    let opts = BbCodeOptions::default();
    let ast = parse_bbcode_to_ast("[b]Bold[/b]", &opts).unwrap();
    assert_eq!(ast.len(), 1);

    // 最初のノードが Bold であることを確認
    match &ast[0] {
        Node::Element(el) => {
            assert_eq!(el.name, "b");
            assert_eq!(el.children.len(), 1);
            assert_eq!(el.children[0], Node::Text("Bold".to_string()));
        }
        _ => panic!("Expected Bold node"),
    }
}

#[test]
fn test_color_valid() {
    let opts = BbCodeOptions::default();
    let input = "[color=red]赤文字[/color]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();
    assert_eq!(ast.len(), 1);

    match &ast[0] {
        Node::Element(el) => {
            assert_eq!(el.name, "color");
            assert_eq!(el.attrs.len(), 1);
            assert_eq!(el.attrs[0].0, "value");
            assert_eq!(el.attrs[0].1, "red");
            assert_eq!(el.children.len(), 1);
        }
        _ => panic!("Expected Color node"),
    }
}

#[test]
fn test_color_invalid() {
    let opts = BbCodeOptions::default();
    let input = "[color=javascript:alert(1)]hack[/color]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();
    // xssが疑われる不正な color は Text に fallback
    match &ast[0] {
        Node::Text(raw) => {
            assert!(raw.contains("hack"), "Should contain original text");
            assert!(
                raw.contains("javascript"),
                "Should keep original invalid value"
            );
        }
        _ => panic!("Expected UnknownTag for invalid color"),
    }
}

#[test]
fn test_nest_depth_exceeded() {
    let opts = BbCodeOptions {
        max_depth: 2,
        ..Default::default()
    };
    // 3階層のネスト
    let input = "[b][i][color=red]Nested[/color][/i][/b]";
    let result = parse_bbcode_to_ast(input, &opts);

    match result {
        Err(BbCodeError::NestDepthExceeded { max_depth, near }) => {
            assert_eq!(max_depth, 2);
            // どのタグ付近で落ちたかは実装依存になり得るので、最低限の確認に留める
            assert!(
                near.contains("["),
                "near should contain some tag-related snippet"
            );
        }
        _ => panic!("Expected NestDepthExceeded error"),
    }
}

#[test]
fn test_generate_html() {
    let opts = BbCodeOptions::default();
    let ast = parse_bbcode_to_ast("[b]Bold[/b]", &opts).unwrap();
    let html = ast_to_html(&ast);
    assert_eq!(html, "<b>Bold</b>");
}

#[test]
fn test_input_size_exceeded() {
    let opts = BbCodeOptions {
        max_input_size: 10, // 10byte
        ..Default::default()
    };
    let long_input = "a".repeat(50); // 50byte
    let result = parse_bbcode_to_ast(&long_input, &opts);
    match result {
        Err(BbCodeError::InputSizeExceeded {
            max_size,
            actual_size,
        }) => {
            assert_eq!(max_size, 10);
            assert_eq!(actual_size, 50);
        }
        _ => panic!("Expected InputSizeExceeded error"),
    }
}

#[test]
fn test_tag_count_exceeded() {
    let opts = BbCodeOptions {
        max_tags: 2,
        ..Default::default()
    };
    // 3つのタグ
    let input = "[b][i][color=red]three tags[/color][/i][/b]";
    let result = parse_bbcode_to_ast(input, &opts);
    match result {
        Err(BbCodeError::TagCountExceeded { max_tags }) => {
            assert_eq!(max_tags, 2);
        }
        _ => panic!("Expected TagCountExceeded error"),
    }
}

#[test]
fn test_mismatched_tags() {
    let opts = BbCodeOptions::default();
    // [b]...[/i] のように異なるタグ名で閉じる
    let input = "[b]Hello[/i]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    // 不整合時はTextにfallback
    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Text(raw) => {
            assert!(raw.contains("Hello"));
            assert!(raw.contains("[b]"));
            assert!(raw.contains("[/i]"));
        }
        _ => panic!("Expected UnknownTag for mismatched tags"),
    }
}

#[test]
fn test_newline_to_br() {
    let opts = BbCodeOptions::default();
    let input = "Hello\nWorld";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    // ASTは 1ノード (Text("Hello\nWorld"))
    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Text(txt) => assert_eq!(txt, "Hello\nWorld"),
        _ => panic!("Expected a single Text node"),
    }

    // HTML化すると改行が <br> に
    let html = ast_to_html(&ast);
    assert_eq!(html, "Hello<br>World");
}

#[test]
fn test_unclosed_tag_fallback() {
    let opts = BbCodeOptions::default();
    let input = "[b]Unclosed bold";

    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    // normalize_text_nodes があるので 1ノードにまとまる
    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Text(raw) => assert_eq!(raw, input),
        _ => panic!("Expected Text fallback for unclosed tag"),
    }
}

#[test]
fn test_unclosed_tag_does_not_break_following_tags() {
    let opts = BbCodeOptions::default();
    let input = "[b]hello [i]world[/i]";

    let ast = parse_bbcode_to_ast(input, &opts).unwrap();
    // 期待：先頭の [b] は Text 化、[i]...[/i] は生きる
    // ただし normalize で Text がまとまる可能性あり

    let html = ast_to_html(&ast);
    assert_eq!(html, "[b]hello <i>world</i>");
}

#[test]
fn test_color_hash_six_digits() {
    let opts = BbCodeOptions::default();
    let input = "[color=#123ABC]Test[/color]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Element(el) => {
            assert_eq!(el.name, "color");
            assert_eq!(el.attrs.len(), 1);
            assert_eq!(el.attrs[0].0, "value");
            assert_eq!(el.attrs[0].1, "#123ABC");
            assert_eq!(el.children.len(), 1);
            match &el.children[0] {
                Node::Text(txt) => assert_eq!(txt, "Test"),
                _ => panic!("Expected Text inside color"),
            }
        }
        _ => panic!("Expected Element(color) node"),
    }
}

#[test]
fn test_empty_tag_content() {
    let opts = BbCodeOptions::default();
    let input = "[b][/b]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Element(el) => {
            assert_eq!(el.name, "b");
            assert!(
                el.children.is_empty(),
                "Empty content should produce an empty children list"
            );
        }
        _ => panic!("Expected Element(b) node"),
    }
}

#[test]
fn test_pest_parse_error_for_lone_bracket() {
    let opts = BbCodeOptions::default();
    let input = "["; // unclosed_tag にも text にもならない

    let result = parse_bbcode_to_ast(input, &opts);

    match result {
        Err(BbCodeError::PestError(_)) => {}
        _ => panic!("Expected PestError for lone '['"),
    }
}

#[test]
fn test_unknown_tag_fallback_to_text() {
    let opts = BbCodeOptions::default();
    let input = "[foo]hello [i]world[/i][/foo]";

    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Text(raw) => assert_eq!(raw, input),
        _ => panic!("Expected Text fallback for unknown tag"),
    }
}
