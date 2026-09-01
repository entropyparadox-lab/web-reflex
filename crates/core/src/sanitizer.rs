use regex::Regex;
use scraper::{Html, Node};
use std::sync::LazyLock;

static DYNAMIC_CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(^css-[a-z0-9_-]+$|^tw-[a-z0-9_-]+$|^sc-[a-z0-9_-]+$|^_[a-z0-9_-]{4,}$|^[a-z0-9_-]{8,}$|^[a-z0-9]*\d+[a-z0-9]*$)")
        .expect("Valid regex")
});

static DYNAMIC_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(^[0-9]+$|[0-9a-f]{8,}|[a-z0-9_-]{12,}|uuid-[a-z0-9-]+)").expect("Valid regex")
});

static PII_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+|\b\d{2,4}[- ]?\d{3,4}[- ]?\d{4}\b|\b\d{13,16}\b)")
        .expect("Valid regex")
});

#[derive(Debug, Clone)]
pub struct SanitizedElement {
    pub tag: String,
    pub role: Option<String>,
    pub name: Option<String>,
    pub input_type: Option<String>,
    pub aria_label: Option<String>,
    pub clean_id: Option<String>,
    pub clean_classes: Vec<String>,
    pub data_state: Option<String>,
    pub aria_expanded: Option<String>,
    pub disabled: bool,
    pub children: Vec<SanitizedElement>,
}

pub struct DomSanitizer;

impl DomSanitizer {
    pub fn sanitize_html(html: &str) -> Vec<SanitizedElement> {
        let document = Html::parse_document(html);
        let root = document.root_element();
        let mut results = Vec::new();

        for child in root.children() {
            if let Some(elem) = Self::process_node(&child) {
                results.push(elem);
            }
        }
        results
    }

    fn process_node(node: &ego_tree::NodeRef<Node>) -> Option<SanitizedElement> {
        match node.value() {
            Node::Element(elem) => {
                let tag = elem.name().to_lowercase();

                // Filter out non-semantic/junk tags
                if matches!(
                    tag.as_str(),
                    "script" | "style" | "noscript" | "svg" | "path" | "iframe" | "link" | "meta"
                ) {
                    return None;
                }

                let role = elem.attr("role").map(|s| s.to_string());
                let name = elem.attr("name").map(|s| s.to_string());
                let input_type = elem.attr("type").map(|s| s.to_string());

                let aria_label = elem
                    .attr("aria-label")
                    .or_else(|| elem.attr("aria-labelledby"))
                    .or_else(|| elem.attr("title"))
                    .map(|s| PII_RE.replace_all(s, "[PII]").to_string());

                // Sanitize ID
                let clean_id = elem.attr("id").and_then(|id| {
                    if DYNAMIC_ID_RE.is_match(id) {
                        None
                    } else {
                        Some(id.to_string())
                    }
                });

                // Sanitize classes: remove dynamic hashes, keep semantic modifiers
                let mut clean_classes = Vec::new();
                if let Some(classes) = elem.attr("class") {
                    for cls in classes.split_whitespace() {
                        if !DYNAMIC_CLASS_RE.is_match(cls) && cls.len() < 30 {
                            clean_classes.push(cls.to_string());
                        }
                    }
                    clean_classes.sort();
                }

                // Capture semantic UI state (Radix/shadcn data-state, aria-expanded, disabled)
                let data_state = elem
                    .attr("data-state")
                    .or_else(|| elem.attr("data-expanded"))
                    .or_else(|| elem.attr("data-selected"))
                    .or_else(|| elem.attr("data-checked"))
                    .map(|s| s.to_string());

                let aria_expanded = elem
                    .attr("aria-expanded")
                    .or_else(|| elem.attr("aria-selected"))
                    .or_else(|| elem.attr("aria-checked"))
                    .map(|s| s.to_string());

                let disabled =
                    elem.attr("disabled").is_some() || elem.attr("aria-disabled") == Some("true");

                let mut children = Vec::new();
                for child_ref in node.children() {
                    if let Some(child_elem) = Self::process_node(&child_ref) {
                        children.push(child_elem);
                    }
                }

                // If non-interactive container with no semantic attrs and no semantic children, prune it
                let is_interactive = matches!(
                    tag.as_str(),
                    "button" | "input" | "select" | "option" | "textarea" | "a" | "form"
                ) || role.is_some()
                    || aria_label.is_some()
                    || data_state.is_some()
                    || aria_expanded.is_some()
                    || disabled;

                if !is_interactive
                    && clean_classes.is_empty()
                    && clean_id.is_none()
                    && children.is_empty()
                {
                    return None;
                }

                Some(SanitizedElement {
                    tag,
                    role,
                    name,
                    input_type,
                    aria_label,
                    clean_id,
                    clean_classes,
                    data_state,
                    aria_expanded,
                    disabled,
                    children,
                })
            }
            _ => None,
        }
    }
}
