use crate::constants::MAX_DOC_SIZE;
use crate::errors::NodecosmosError;
use ammonia::{clean, Builder};

pub trait Clean {
    fn clean(&mut self) -> Result<&mut Self, NodecosmosError>;

    fn clean_clone(&self) -> Self
    where
        Self: Sized + Clone,
    {
        let mut cloned = self.clone();
        let _ = cloned.clean().map_err(|e| {
            log::error!("{}", e);
            NodecosmosError::InternalServerError
        });

        cloned
    }
}

impl Clean for Option<String> {
    fn clean(&mut self) -> Result<&mut Self, NodecosmosError> {
        if let Some(description) = &self {
            if description.len() > MAX_DOC_SIZE {
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

        Ok(self)
    }
}

impl Clean for String {
    fn clean(&mut self) -> Result<&mut Self, NodecosmosError> {
        if self.len() > MAX_DOC_SIZE {
            return Err(NodecosmosError::Forbidden(
                "Data is too long. It can contain max 16MB of data".to_string(),
            ));
        }

        *self = clean(self);

        Ok(self)
    }
}
