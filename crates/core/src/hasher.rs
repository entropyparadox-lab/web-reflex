use crate::sanitizer::{DomSanitizer, SanitizedElement};
use sha2::{Digest, Sha256};

pub struct SkeletonHasher;

impl SkeletonHasher {
    pub fn compute_hash(html: &str) -> String {
        let elements = DomSanitizer::sanitize_html(html);
        let canonical_str = Self::serialize_elements(&elements);

        let mut hasher = Sha256::new();
        hasher.update(canonical_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn serialize_elements(elements: &[SanitizedElement]) -> String {
        let mut buf = String::new();
        for elem in elements {
            Self::write_element(elem, &mut buf);
        }
        buf
    }

    fn write_element(elem: &SanitizedElement, buf: &mut String) {
        buf.push('<');
        buf.push_str(&elem.tag);

        if let Some(role) = &elem.role {
            buf.push_str("[role=");
            buf.push_str(role);
            buf.push(']');
        }

        if let Some(itype) = &elem.input_type {
            buf.push_str("[type=");
            buf.push_str(itype);
            buf.push(']');
        }

        if let Some(name) = &elem.name {
            buf.push_str("[name=");
            buf.push_str(name);
            buf.push(']');
        }

        if let Some(clean_id) = &elem.clean_id {
            buf.push_str("[id=");
            buf.push_str(clean_id);
            buf.push(']');
        }

        if !elem.clean_classes.is_empty() {
            buf.push_str("[cls=");
            buf.push_str(&elem.clean_classes.join(","));
            buf.push(']');
        }

        if let Some(aria) = &elem.aria_label {
            buf.push_str("[aria=");
            buf.push_str(aria);
            buf.push(']');
        }

        if let Some(state) = &elem.data_state {
            buf.push_str("[state=");
            buf.push_str(state);
            buf.push(']');
        }

        if let Some(exp) = &elem.aria_expanded {
            buf.push_str("[exp=");
            buf.push_str(exp);
            buf.push(']');
        }

        if elem.disabled {
            buf.push_str("[disabled]");
        }

        buf.push('>');

        for child in &elem.children {
            Self::write_element(child, buf);
        }

        buf.push_str("</");
        buf.push_str(&elem.tag);
        buf.push('>');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeleton_hash_stability() {
        let html1 = r#"
            <div class="css-1a2b3c tw-flex">
                <form id="login-form">
                    <input type="text" name="username" value="user@example.com" class="input-primary _89df2" />
                    <input type="password" name="password" value="secret123" class="input-primary" />
                    <button type="submit" aria-label="로그인 하기" class="css-btn-active btn-submit">로그인</button>
                </form>
            </div>
        "#;

        let html2 = r#"
            <div class="css-9z8y7x tw-flex">
                <form id="login-form">
                    <input type="text" name="username" value="other_user@test.org" class="input-primary _34bc1" />
                    <input type="password" name="password" value="different_pwd" class="input-primary" />
                    <button type="submit" aria-label="로그인 하기" class="css-btn-other btn-submit">로그인</button>
                </form>
            </div>
        "#;

        let hash1 = SkeletonHasher::compute_hash(html1);
        let hash2 = SkeletonHasher::compute_hash(html2);

        assert_eq!(
            hash1, hash2,
            "Hashes should be identical despite dynamic classes and values"
        );
    }

    #[test]
    fn test_skeleton_hash_changes_on_structural_diff() {
        let html1 = r#"
            <div>
                <button type="submit">Submit</button>
            </div>
        "#;

        let html2 = r#"
            <div>
                <input type="text" name="extra" />
                <button type="submit">Submit</button>
            </div>
        "#;

        let hash1 = SkeletonHasher::compute_hash(html1);
        let hash2 = SkeletonHasher::compute_hash(html2);

        assert_ne!(
            hash1, hash2,
            "Structural difference must yield different hashes"
        );
    }

    #[test]
    fn test_skeleton_hash_changes_on_modal_state_toggle() {
        let modal_closed = r#"
            <div role="dialog" data-state="closed" aria-expanded="false">
                <form>
                    <input type="text" name="order_id" />
                    <button type="submit">Confirm</button>
                </form>
            </div>
        "#;

        let modal_open = r#"
            <div role="dialog" data-state="open" aria-expanded="true">
                <form>
                    <input type="text" name="order_id" />
                    <button type="submit">Confirm</button>
                </form>
            </div>
        "#;

        let hash_closed = SkeletonHasher::compute_hash(modal_closed);
        let hash_open = SkeletonHasher::compute_hash(modal_open);

        assert_ne!(
            hash_closed, hash_open,
            "data-state='closed' vs 'open' must yield different skeleton hashes to prevent modal action collision"
        );
    }

    #[test]
    fn test_skeleton_hash_changes_on_disabled_state() {
        let button_enabled = r#"<button type="submit" class="btn-primary">Submit</button>"#;
        let button_disabled =
            r#"<button type="submit" disabled class="btn-primary">Submit</button>"#;

        let hash_enabled = SkeletonHasher::compute_hash(button_enabled);
        let hash_disabled = SkeletonHasher::compute_hash(button_disabled);

        assert_ne!(
            hash_enabled, hash_disabled,
            "Enabled vs disabled button states must yield distinct hashes"
        );
    }
}
