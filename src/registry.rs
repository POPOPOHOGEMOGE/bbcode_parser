use once_cell::sync::Lazy;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct TagSpec {
    /// `[color=xxx]` のように 1つの “値属性” を許可するか
    pub allow_value_attr: bool,
    /// 値属性を検証する（colorのようなケース）
    pub validate_value_attr: Option<fn(&str) -> bool>,
}

impl TagSpec {
    pub fn simple() -> Self {
        Self {
            allow_value_attr: false,
            validate_value_attr: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TagRegistry;

impl TagRegistry {
    /// “このタグは何か？”（仕様）を返す
    pub fn get(tag_name: &str) -> Option<TagSpec> {
        match tag_name.to_ascii_lowercase().as_str() {
            "b" => Some(TagSpec::simple()),
            "i" => Some(TagSpec::simple()),
            "color" => Some(TagSpec {
                allow_value_attr: true,
                validate_value_attr: Some(is_valid_color_value),
            }),
            _ => None,
        }
    }
}

/// 英字 or #RGB or #RRGGBB
fn is_valid_color_value(s: &str) -> bool {
    static COLOR_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^([A-Za-z]+|#[0-9A-Fa-f]{3}([0-9A-Fa-f]{3})?)$")
            .expect("color regex must be valid")
    });
    COLOR_RE.is_match(s.trim())
}
