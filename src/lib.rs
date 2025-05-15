use once_cell::sync::Lazy;
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;
use thiserror::Error;

// ========== 1) パーサ定義 (Pest) ========== //
#[derive(Parser)]
#[grammar = "bbcode.pest"]
struct BBCodeParser;

// ========== 2) 公開API用の設定構造体・エラー定義 ========== //

/// BBCode パーサの各種制限設定
#[derive(Debug, Clone)]
pub struct BbCodeOptions {
    /// ネストできる最大深度 (これを超えるとエラー)
    pub max_depth: usize,
    /// タグ数の上限 (これを超えるとエラー)
    pub max_tags: usize,
    /// 入力文字列の最大サイズ (バイト)
    pub max_input_size: usize,
}

impl Default for BbCodeOptions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_tags: 500,
            max_input_size: 50 * 1024, // 50KB
        }
    }
}

/// パース時に発生しうるエラー
#[derive(Debug, Error)]
pub enum BbCodeError {
    #[error("Input size exceeded limit (max {max_size} bytes)")]
    InputSizeExceeded { max_size: usize, actual_size: usize },
    #[error("Parsed tag count exceeded limit (max {max_tags})")]
    TagCountExceeded { max_tags: usize },

    #[error("Nest depth exceeded limit (max {max_depth}). Near: \"{near}\"")]
    NestDepthExceeded { max_depth: usize, near: String },

    #[error("Failed to parse input: {0}")]
    PestError(#[from] pest::error::Error<Rule>),
}

// ========== 3) AST 定義 ========== //

/// BBCode の抽象構文木
#[derive(Debug, Clone)]
pub enum Node {
    Text(String),
    Bold(Vec<Node>),
    Italic(Vec<Node>),
    Color(String, Vec<Node>), // color値, 中身
    /// 未知タグ: 入力文字列をそのまま保存
    UnknownTag(String),
}

/// カラー値チェック用の正規表現 (英字 or #RGB/#RRGGBB)
///   - 英字: [a-zA-Z]+
///   - #RGB or #RRGGBB: ^#[0-9A-Fa-f]{3}([0-9A-Fa-f]{3})?$
fn is_valid_color_value(s: &str) -> bool {
    static COLOR_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^([a-zA-Z]+|#[0-9A-Fa-f]{3,6})$").unwrap());
    COLOR_RE.is_match(s)
}

// ========== 4) 公開関数：BBCode をパースして AST を返す ========== //

/// 入力文字列を BBCode AST にパース
///
/// 要件:
///   - 入力サイズチェック
///   - pestでパース
///   - AST構築時にタグ数/深度チェック
pub fn parse_bbcode_to_ast(input: &str, opts: &BbCodeOptions) -> Result<Vec<Node>, BbCodeError> {
    // 1) 入力サイズチェック
    let bytes_len = input.len();
    if bytes_len > opts.max_input_size {
        return Err(BbCodeError::InputSizeExceeded {
            max_size: opts.max_input_size,
            actual_size: bytes_len,
        });
    }

    // 2) pestでパース
    let pairs = BBCodeParser::parse(Rule::BBCode, input)?; // → PestError が起きる可能性

    // 3) ASTを構築 (タグ数 & 深度 管理のため、別途コンテキストを用意)
    let mut ctx = BuildAstContext::new(opts);
    let mut ast = vec![];

    for pair in pairs {
        ast.extend(ctx.build_ast(pair, 0)?);
    }

    Ok(ast)
}

/// AST→HTML 変換 (簡易実装)
/// - color 属性チェック & エスケープ
/// - ネスト深度超過時点で parse_bbcode_to_ast() がエラーになるため、ここでは深度チェック不要
pub fn ast_to_html(nodes: &[Node]) -> String {
    let mut result = String::new();

    for node in nodes {
        match node {
            Node::Text(txt) => {
                // 改行→<br> / HTMLエスケープ
                let escaped = escape_html(txt);
                let replaced = replace_newline_with_br(&escaped);
                result.push_str(&replaced);
            }
            Node::Bold(children) => {
                result.push_str("<b>");
                result.push_str(&ast_to_html(children));
                result.push_str("</b>");
            }
            Node::Italic(children) => {
                result.push_str("<i>");
                result.push_str(&ast_to_html(children));
                result.push_str("</i>");
            }
            Node::Color(color_val, children) => {
                // color_val の安全性はパース時点でチェックしているが、念のためエスケープ
                let escaped_color = escape_html(color_val);
                result.push_str("<span style=\"color:");
                result.push_str(&escaped_color);
                result.push_str("\">");
                result.push_str(&ast_to_html(children));
                result.push_str("</span>");
            }
            Node::UnknownTag(raw) => {
                // 丸ごとテキストとして表示
                let replaced = replace_newline_with_br(raw);
                let escaped = escape_html(&replaced);
                result.push_str(&escaped);
            }
        }
    }

    result
}

// ========== 5) AST構築の内部実装 ========== //

/// AST 構築時に必要なコンテキスト (オプション、タグ数カウント)
struct BuildAstContext<'a> {
    opts: &'a BbCodeOptions,
    tag_count: usize,
}

impl<'a> BuildAstContext<'a> {
    fn new(opts: &'a BbCodeOptions) -> Self {
        Self { opts, tag_count: 0 }
    }

    /// 再帰的に Pair(一つの要素) から Node群を構築
    fn build_ast(
        &mut self,
        pair: pest::iterators::Pair<Rule>,
        depth: usize,
    ) -> Result<Vec<Node>, BbCodeError> {
        match pair.as_rule() {
            Rule::BBCode => {
                // ルート: content*
                let mut result = vec![];
                for inner in pair.into_inner() {
                    result.extend(self.build_ast(inner, depth)?);
                }
                Ok(result)
            }
            Rule::content => {
                // content は tag_block | escaped_bracket | text
                let mut result = vec![];
                for inner in pair.into_inner() {
                    result.extend(self.build_ast(inner, depth)?);
                }
                Ok(result)
            }
            Rule::tag_block => {
                // [tag_name (+attr?)] content... [/close_tag_name]
                // まず depth チェック
                if depth >= self.opts.max_depth {
                    // エラー扱い (フォールバックせずエラー返す)
                    let near = pair.as_str().to_string();
                    return Err(BbCodeError::NestDepthExceeded {
                        max_depth: self.opts.max_depth,
                        near,
                    });
                }

                // タグ数カウント
                self.tag_count += 1;
                if self.tag_count > self.opts.max_tags {
                    return Err(BbCodeError::TagCountExceeded {
                        max_tags: self.opts.max_tags,
                    });
                }

                // 抽出: [tag_name + tag_attr?]
                let mut inner = pair.into_inner();

                // 1) 開始タグ
                let open_pair = inner.next().unwrap(); // tag_name
                let tag_name_str = open_pair.as_str();

                // 2) optional attr?
                //   pestのルール上、あれば次のペアが tag_attr
                let mut attr_val: Option<String> = None;
                if let Some(next_pair) = inner.peek() {
                    if next_pair.as_rule() == Rule::tag_attr {
                        let attr_pair = inner.next().unwrap();
                        let raw_attr = attr_pair.as_str(); // "=xxxx"
                                                           // 先頭の '=' を外した後が実際の値
                        let actual = &raw_attr[1..];
                        attr_val = Some(actual.to_string());
                    }
                }

                // 3) 中身(content*)
                let mut children = vec![];
                let mut content_pairs = vec![];
                // children部分は可変個だからどこまで？
                // grammarで content* → [/] が来るまで
                loop {
                    let peeked = inner.peek();
                    if let Some(p) = peeked {
                        // 次がclose_tag_nameならbreak
                        if p.as_rule() == Rule::close_tag_name {
                            break;
                        }
                        // それ以外はcontentとみなす
                        content_pairs.push(inner.next().unwrap());
                    } else {
                        // 終わり
                        break;
                    }
                }

                // 4) 閉じタグ
                let close_tag_pair = inner.next().unwrap(); // close_tag_name
                let close_tag_name_str = close_tag_pair.as_str();

                // もし open_tag_name != close_tag_name なら不整合 → フォールバック
                if tag_name_str != close_tag_name_str {
                    // 不整合が発覚 → 丸ごとテキストに戻す
                    let fallback_text = format!(
                        "[{}{}]{}[/{}]",
                        tag_name_str,
                        attr_val
                            .as_ref()
                            .map(|av| format!("={}", av))
                            .unwrap_or_default(),
                        content_pairs
                            .iter()
                            .map(|p| p.as_str())
                            .collect::<Vec<_>>()
                            .join(""),
                        close_tag_name_str
                    );
                    return Ok(vec![Node::UnknownTag(fallback_text)]);
                }

                // 子要素を再帰パース
                for cp in content_pairs {
                    children.extend(self.build_ast(cp, depth + 1)?);
                }

                // 5) タグ名ごとに分岐
                let lower_name = tag_name_str.to_lowercase();
                match lower_name.as_str() {
                    "b" => Ok(vec![Node::Bold(children)]),
                    "i" => Ok(vec![Node::Italic(children)]),
                    "color" => {
                        // color=xxxx のチェック
                        if let Some(val) = attr_val {
                            let trimmed = val.trim();
                            if is_valid_color_value(trimmed) {
                                Ok(vec![Node::Color(trimmed.to_string(), children)])
                            } else {
                                // 不正 => 丸ごとフォールバック
                                let fallback_text = format!(
                                    "[color={}]{text}[/color]",
                                    trimmed,
                                    text = children_to_text(&children)
                                );
                                Ok(vec![Node::UnknownTag(fallback_text)])
                            }
                        } else {
                            // color= がない → フォールバック
                            let fallback_text = format!(
                                "[color]{text}[/color]",
                                text = children_to_text(&children)
                            );
                            Ok(vec![Node::UnknownTag(fallback_text)])
                        }
                    }
                    // 未知タグ => フォールバック
                    _ => {
                        let fallback_text = format!(
                            "[{}{}]{}[/{}]",
                            tag_name_str,
                            attr_val
                                .as_ref()
                                .map(|av| format!("={}", av))
                                .unwrap_or_default(),
                            children_to_text(&children),
                            close_tag_name_str
                        );
                        Ok(vec![Node::UnknownTag(fallback_text)])
                    }
                }
            }
            Rule::escaped_bracket => {
                // \[ → "["
                Ok(vec![Node::Text("[".to_string())])
            }
            Rule::text => Ok(vec![Node::Text(pair.as_str().to_string())]),
            Rule::EOI => {
                // 終端マーカーは無視（Nodeは生成しない）
                Ok(vec![])
            }
            _ => {
                // 予期しないルールはそのままテキスト化
                Ok(vec![Node::Text(pair.as_str().to_string())])
            }
        }
    }
}

// ========== 6) 補助関数 ========== //

/// ノード列をそのままテキスト化 (フォールバック用)
fn children_to_text(nodes: &[Node]) -> String {
    let mut buf = String::new();
    for n in nodes {
        match n {
            Node::Text(t) => buf.push_str(t),
            Node::Bold(child) => {
                buf.push_str("[b]");
                buf.push_str(&children_to_text(child));
                buf.push_str("[/b]");
            }
            Node::Italic(child) => {
                buf.push_str("[i]");
                buf.push_str(&children_to_text(child));
                buf.push_str("[/i]");
            }
            Node::Color(c, child) => {
                buf.push_str(&format!("[color={}]", c));
                buf.push_str(&children_to_text(child));
                buf.push_str("[/color]");
            }
            Node::UnknownTag(u) => buf.push_str(u),
        }
    }
    buf
}

/// HTML用エスケープ (最低限)
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 改行を <br> に置換
fn replace_newline_with_br(input: &str) -> String {
    // CRLF→LF / CR→LF に置換後、LFを<br>に
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "<br>")
}
