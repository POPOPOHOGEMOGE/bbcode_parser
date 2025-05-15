use bbcode_parser::{ast_to_html, parse_bbcode_to_ast, BbCodeError, BbCodeOptions, Node};

#[test]
fn test_basic_parse() {
    let opts = BbCodeOptions::default();
    let ast = parse_bbcode_to_ast("[b]Bold[/b]", &opts).unwrap();
    assert_eq!(ast.len(), 1);

    // 最初のノードが Bold であることを確認
    match &ast[0] {
        Node::Bold(children) => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                Node::Text(txt) => assert_eq!(txt, "Bold"),
                _ => panic!("Expected text inside bold"),
            }
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
        Node::Color(c, children) => {
            assert_eq!(c, "red");
            assert_eq!(children.len(), 1);
        }
        _ => panic!("Expected Color node"),
    }
}

#[test]
fn test_color_invalid() {
    let opts = BbCodeOptions::default();
    let input = "[color=javascript:alert(1)]hack[/color]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();
    // xssが疑われる不正な color は UnknownTag として扱う
    match &ast[0] {
        Node::UnknownTag(raw) => {
            assert!(raw.contains("hack"), "Should contain original text");
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
            // nearにパース失敗箇所の文字列が含まれる
            assert!(
                near.contains("[color=red]"),
                "Should mention the third-level tag"
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

    // 不整合時はフォールバックで UnknownTag になる
    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::UnknownTag(raw) => {
            assert!(
                raw.contains("Hello"),
                "Fallback text should contain original content"
            );
            assert!(
                raw.contains("[b]"),
                "Should contain the original opening tag"
            );
            assert!(
                raw.contains("[/i]"),
                "Should contain the original closing tag"
            );
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
fn test_pest_parse_error() {
    let opts = BbCodeOptions::default();
    // 閉じタグなし => Pestのルール上パースエラーが発生
    let input = "[b]Unclosed bold";
    let result = parse_bbcode_to_ast(input, &opts);

    match result {
        Err(BbCodeError::PestError(_)) => {
            // Pestエラーとして処理
        }
        _ => panic!("Expected PestError for unclosed tag"),
    }
}

#[test]
fn test_color_hash_six_digits() {
    let opts = BbCodeOptions::default();
    let input = "[color=#123ABC]Test[/color]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Color(c, children) => {
            assert_eq!(c, "#123ABC");
            assert_eq!(children.len(), 1);
            if let Node::Text(txt) = &children[0] {
                assert_eq!(txt, "Test");
            } else {
                panic!("Expected Text node inside color");
            }
        }
        _ => panic!("Expected Color node"),
    }
}

#[test]
fn test_empty_tag_content() {
    let opts = BbCodeOptions::default();
    let input = "[b][/b]";
    let ast = parse_bbcode_to_ast(input, &opts).unwrap();

    assert_eq!(ast.len(), 1);
    match &ast[0] {
        Node::Bold(children) => {
            assert_eq!(
                children.len(),
                0,
                "Empty content should produce an empty children list"
            );
        }
        _ => panic!("Expected Bold node"),
    }
}
