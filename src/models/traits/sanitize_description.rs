use crate::errors::NodecosmosError;
use ammonia::{clean, Builder};

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

            let mut sanitizer = Builder::default();
            sanitizer
                .add_tag_attributes("img", &["resizable"])
                .add_tag_attributes("code", &["spellcheck"]);

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
