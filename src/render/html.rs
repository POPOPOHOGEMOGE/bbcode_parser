use crate::ast::{Element, Node};
use crate::registry::TagRegistry;

pub fn ast_to_html(nodes: &[Node]) -> String {
    let mut out = String::new();
    for n in nodes {
        render_node(n, &mut out);
    }
    out
}

fn render_node(node: &Node, out: &mut String) {
    match node {
        Node::Text(txt) => {
            let escaped = escape_html(txt);
            let replaced = replace_newline_with_br(&escaped);
            out.push_str(&replaced);
        }
        Node::Element(el) => render_element(el, out),
    }
}

fn render_element(el: &Element, out: &mut String) {
    // tag spec が無い = unknown tag
    let Some(_spec) = TagRegistry::get(&el.name) else {
        // unknown tag: タグ自体は捨てて中身だけ表示
        for c in &el.children {
            render_node(c, out);
        }
        return;
    };

    match el.name.as_str() {
        "b" => {
            out.push_str("<b>");
            for c in &el.children {
                render_node(c, out);
            }
            out.push_str("</b>");
        }
        "i" => {
            out.push_str("<i>");
            for c in &el.children {
                render_node(c, out);
            }
            out.push_str("</i>");
        }
        "color" => {
            // attrs["value"] を探す（parserが正規化済み）
            let value = el
                .attrs
                .iter()
                .find(|(k, _)| k == "value")
                .map(|(_, v)| v.as_str());

            // valueが無いならタグを無視して中身だけ（安全寄り）
            let Some(color_val) = value else {
                for c in &el.children {
                    render_node(c, out);
                }
                return;
            };

            // 念のため再検証（render層で二重に守る）
            if let Some(spec) = TagRegistry::get("color") {
                if let Some(vfn) = spec.validate_value_attr {
                    if !vfn(color_val) {
                        for c in &el.children {
                            render_node(c, out);
                        }
                        return;
                    }
                }
            }

            let escaped_color = escape_html(color_val);
            out.push_str("<span style=\"color:");
            out.push_str(&escaped_color);
            out.push_str("\">");
            for c in &el.children {
                render_node(c, out);
            }
            out.push_str("</span>");
        }
        _ => {
            // registryで unknown 扱いにしないならここは基本来ない
            for c in &el.children {
                render_node(c, out);
            }
        }
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn replace_newline_with_br(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "<br>")
}
