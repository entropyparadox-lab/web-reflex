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
                    <input type="text" name="username" value="other_person@gmail.com" class="input-primary _33aa1" />
                    <input type="password" name="password" value="different_pwd" class="input-primary" />
                    <button type="submit" aria-label="로그인 하기" class="css-btn-hovered btn-submit">로그인</button>
                </form>
            </div>
        "#;

        let hash1 = SkeletonHasher::compute_hash(html1);
        let hash2 = SkeletonHasher::compute_hash(html2);

        assert_eq!(
            hash1, hash2,
            "Dynamic CSS and user values must not alter the skeleton hash!"
        );
    }

    #[test]
    fn test_skeleton_hash_changes_on_structural_diff() {
        let html1 = r#"
            <form id="login-form">
                <input type="text" name="username" />
                <button type="submit">Submit</button>
            </form>
        "#;

        let html2 = r#"
            <form id="login-form">
                <input type="text" name="username" />
                <input type="text" name="otp_code" />
                <button type="submit">Submit</button>
            </form>
        "#;

        let hash1 = SkeletonHasher::compute_hash(html1);
        let hash2 = SkeletonHasher::compute_hash(html2);

        assert_ne!(
            hash1, hash2,
            "Structural change must result in different skeleton hash!"
        );
    }
}
