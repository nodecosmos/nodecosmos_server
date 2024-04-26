use ammonia::{clean, Builder};

use crate::errors::NodecosmosError;

pub trait SanitizeDescription {
    fn sanitize(&mut self) -> Result<(), NodecosmosError>;
}

impl SanitizeDescription for Option<String> {
    fn sanitize(&mut self) -> Result<(), NodecosmosError> {
        if let Some(description) = &self {
            if description.len() > u16::MAX as usize {
                return Err(NodecosmosError::Forbidden(
                    "Description is too long. It can contain max 65535 characters".to_string(),
                ));
            }

            // TODO: move this to resources
            let mut sanitizer = Builder::default();

            sanitizer
                .add_tag_attributes("img", &["resizable"])
                .add_tag_attributes("pre", &["spellcheck", "class"])
                .add_tag_attributes("code", &["spellcheck", "data-code-block-language"])
                .set_tag_attribute_value("a", "rel", "noopener noreferrer nofollow")
                .set_tag_attribute_value("a", "target", "_blank")
                .link_rel(None);

            let sanitized = sanitizer.clean(description);

            *self = Some(sanitized.to_string());
        }

        Ok(())
    }
}

impl SanitizeDescription for String {
    fn sanitize(&mut self) -> Result<(), NodecosmosError> {
        if self.len() > u16::MAX as usize {
            return Err(NodecosmosError::Forbidden(
                "Description is too long. It can contain max 65535 characters".to_string(),
            ));
        }

        *self = clean(self);

        Ok(())
    }
}
