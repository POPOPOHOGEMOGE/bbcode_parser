use pest::Parser;
use pest_derive::Parser;

use crate::ast::{Element, Node};
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

    fn check_depth(&self, depth: usize, near: &str) -> Result<(), BbCodeError> {
        if depth >= self.opts.max_depth {
            return Err(BbCodeError::NestDepthExceeded {
                max_depth: self.opts.max_depth,
                near: near.to_string(),
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
                self.check_depth(depth, pair.as_str())?;
                self.on_tag()?;

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
                    return Ok(vec![Node::Text(original)]);
                }

                // 子要素を再帰で構築
                let mut children = vec![];
                for cp in content_pairs {
                    children.extend(self.build_nodes(cp, depth + 1)?);
                }

                // TagSpec に従って属性を許可・検証する
                if let Some(spec) = TagRegistry::get(&open_name_lc) {
                    // 値属性があるのに許可されてない -> フォールバック
                    if value_attr.is_some() && !spec.allow_value_attr {
                        return Ok(vec![Node::Text(original)]);
                    }

                    // 値属性の検証（colorなど）
                    if let (Some(val), Some(validator)) = (&value_attr, spec.validate_value_attr) {
                        if !(validator)(val) {
                            return Ok(vec![Node::Text(original)]);
                        }
                    }
                }
                // unknown tag は「Elementとして残す」（Renderer側でどう出すか決められる）
                let mut elem = Element::new(open_name_lc).with_children(children);

                if let Some(val) = value_attr {
                    // `[color=red]` を attrs=[("value","red")] に正規化
                    elem.attrs
                        .push(("value".to_string(), val.trim().to_string()));
                }

                Ok(vec![Node::Element(elem)])
            }

            Rule::escaped_bracket => Ok(vec![Node::Text("[".to_string())]),
            Rule::text => Ok(vec![Node::Text(pair.as_str().to_string())]),
            Rule::EOI => Ok(vec![]),

            _ => Ok(vec![Node::Text(pair.as_str().to_string())]),
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

/// 隣接 Text をマージして扱いやすくする（地味に効く）
fn normalize_text_nodes(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());

    for n in nodes {
        match (out.last_mut(), n) {
            (Some(Node::Text(prev)), Node::Text(cur)) => prev.push_str(&cur),
            (_, other) => out.push(other),
        }
    }

    out
}
