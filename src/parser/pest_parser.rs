use pest::Parser;
use pest_derive::Parser;

use crate::ast::{Element, Node, Span};
use crate::error::BbCodeError;
use crate::options::BbCodeOptions;
use crate::registry::TagRegistry;

#[derive(Parser)]
#[grammar = "bbcode.pest"]
pub struct BBCodeParser;

/// AST構築時のコンテキスト
struct BuildAstContext<'a> {
    opts: &'a BbCodeOptions,
    tag_count: usize,
}

impl<'a> BuildAstContext<'a> {
    fn new(opts: &'a BbCodeOptions) -> Self {
        Self { opts, tag_count: 0 }
    }

    fn on_tag(&mut self) -> Result<(), BbCodeError> {
        self.tag_count += 1;
        if self.tag_count > self.opts.max_tags {
            return Err(BbCodeError::TagCountExceeded {
                max_tags: self.opts.max_tags,
            });
        }
        Ok(())
    }

    fn check_depth(
        &self,
        depth: usize,
        pair: &pest::iterators::Pair<Rule>,
    ) -> Result<(), BbCodeError> {
        let level = depth.checked_add(1).unwrap_or(usize::MAX);
        if level > self.opts.max_depth {
            let sp = pair.as_span();
            let (line, column) = sp.start_pos().line_col();
            return Err(BbCodeError::NestDepthExceeded {
                max_depth: self.opts.max_depth,
                near: pair.as_str().to_string(),
                span: Span {
                    start: sp.start(),
                    end: sp.end(),
                },
                line,
                column,
            });
        }
        Ok(())
    }

    fn build_nodes(
        &mut self,
        pair: pest::iterators::Pair<Rule>,
        depth: usize,
    ) -> Result<Vec<Node>, BbCodeError> {
        match pair.as_rule() {
            Rule::BBCode | Rule::content => {
                let mut result = vec![];
                for inner in pair.into_inner() {
                    result.extend(self.build_nodes(inner, depth)?);
                }
                Ok(result)
            }

            Rule::tag_block => {
                self.check_depth(depth, &pair)?;
                self.on_tag()?;

                let span = pair_span(&pair);

                let original = pair.as_str().to_string(); // フォールバック用

                let mut inner = pair.into_inner();

                let open_name = inner.next().unwrap().as_str().to_string();
                let open_name_lc = open_name.to_ascii_lowercase();

                // optional: tag_attr (=...)
                let mut value_attr: Option<String> = None;
                if let Some(next) = inner.peek() {
                    if next.as_rule() == Rule::tag_attr {
                        let raw = inner.next().unwrap().as_str(); // "=xxxx"
                        value_attr = Some(raw[1..].to_string());
                    }
                }

                // children (content*) を close_tag_name まで集める
                let mut content_pairs = vec![];
                loop {
                    match inner.peek() {
                        Some(p) if p.as_rule() == Rule::close_tag_name => break,
                        Some(_) => content_pairs.push(inner.next().unwrap()),
                        None => break,
                    }
                }

                let close_name = inner.next().unwrap().as_str().to_string();
                let close_name_lc = close_name.to_ascii_lowercase();

                // タグ不整合は「その部分を丸ごとテキストへ」(構造を壊さない方針)
                if open_name_lc != close_name_lc {
                    return Ok(vec![Node::Text {
                        span,
                        text: original,
                    }]);
                }

                // TagSpec に従って属性を許可・検証する
                // unknown tag は BBCode として扱わない
                let spec = match TagRegistry::get(&open_name_lc) {
                    Some(s) => s,
                    None => {
                        // unknown tag は丸ごとテキストへ（中身も含めて構造化しない）
                        return Ok(vec![Node::Text {
                            span,
                            text: original,
                        }]);
                    }
                };

                // 子要素を再帰で構築
                let mut children = vec![];
                for cp in content_pairs {
                    children.extend(self.build_nodes(cp, depth + 1)?);
                }

                // 値属性があるのに許可されてない -> フォールバック
                if value_attr.is_some() && !spec.allow_value_attr {
                    return Ok(vec![Node::Text {
                        span,
                        text: original,
                    }]);
                }

                // 値属性の検証（colorなど）
                if let (Some(val), Some(validator)) = (&value_attr, spec.validate_value_attr) {
                    if !(validator)(val) {
                        return Ok(vec![Node::Text {
                            span,
                            text: original,
                        }]);
                    }
                }

                let mut elem = Element::new(open_name_lc, span).with_children(children);

                if let Some(val) = value_attr {
                    // `[color=red]` を attrs=[("value","red")] に正規化
                    elem.attrs
                        .push(("value".to_string(), val.trim().to_string()));
                }

                Ok(vec![Node::Element(elem)])
            }

            Rule::unclosed_tag => {
                // 開始タグのみで閉じタグがないケースはその部分を丸ごとテキストへ
                // DoS耐性としてタグ数制限の対象に含める
                self.on_tag()?;
                let span = pair_span(&pair);
                Ok(vec![Node::Text {
                    span,
                    text: pair.as_str().to_string(),
                }])
            }

            Rule::escaped_bracket => {
                let span = pair_span(&pair);
                Ok(vec![Node::Text {
                    span,
                    text: "[".to_string(),
                }])
            }

            Rule::text => {
                let span = pair_span(&pair);
                Ok(vec![Node::Text {
                    span,
                    text: pair.as_str().to_string(),
                }])
            }
            Rule::EOI => Ok(vec![]),

            _ => {
                let span = pair_span(&pair);
                Ok(vec![Node::Text {
                    span,
                    text: pair.as_str().to_string(),
                }])
            }
        }
    }
}

/// 公開API：入力文字列をASTにパース
pub fn parse_bbcode_to_ast(input: &str, opts: &BbCodeOptions) -> Result<Vec<Node>, BbCodeError> {
    if input.len() > opts.max_input_size {
        return Err(BbCodeError::InputSizeExceeded {
            max_size: opts.max_input_size,
            actual_size: input.len(),
        });
    }

    let pairs = BBCodeParser::parse(Rule::BBCode, input)?;
    let mut ctx = BuildAstContext::new(opts);

    let mut nodes = vec![];
    for p in pairs {
        nodes.extend(ctx.build_nodes(p, 0)?);
    }

    Ok(normalize_text_nodes(nodes))
}

/// 隣接 Text をマージして扱いやすくする
fn normalize_text_nodes(nodes: Vec<Node>) -> Vec<Node> {
    let mut normalized: Vec<Node> = Vec::with_capacity(nodes.len());

    for n in nodes {
        match n {
            Node::Text { .. } => normalized.push(n),
            Node::Element(mut el) => {
                el.children = normalize_text_nodes(el.children);
                normalized.push(Node::Element(el));
            }
        }
    }

    let mut out: Vec<Node> = Vec::with_capacity(normalized.len());
    for n in normalized {
        match (out.last_mut(), n) {
            (
                Some(Node::Text {
                    span: prev_span,
                    text: prev_text,
                }),
                Node::Text {
                    span: cur_span,
                    text: cur_text,
                },
            ) => {
                prev_text.push_str(&cur_text);
                prev_span.end = cur_span.end;
            }
            (_, other) => out.push(other),
        }
    }

    out
}

fn pair_span(pair: &pest::iterators::Pair<Rule>) -> Span {
    let sp = pair.as_span();
    Span {
        start: sp.start(),
        end: sp.end(),
    }
}
