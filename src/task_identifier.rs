use std::sync::Arc;

/// Cheaply clonable TaskIdentifier
#[derive(Debug, Clone)]
pub enum TaskIdentifier {
    Literal(&'static str),
    Arc(Arc<String>),
}

impl From<&'static str> for TaskIdentifier {
    fn from(value: &'static str) -> Self {
        Self::Literal(value)
    }
}

impl From<String> for TaskIdentifier {
    fn from(value: String) -> Self {
        Self::Arc(Arc::new(value))
    }
}

impl From<Arc<String>> for TaskIdentifier {
    fn from(value: Arc<String>) -> Self {
        Self::Arc(value.clone())
    }
}

impl std::fmt::Display for TaskIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskIdentifier::Literal(data) => write!(f, "{data}"),
            TaskIdentifier::Arc(data) => write!(f, "{data}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let identifier = TaskIdentifier::from("Hello World");
        assert_eq!(identifier.to_string(), "Hello World");

        let identifier = "Hello World".to_owned();
        let identifier = TaskIdentifier::from(identifier);
        assert_eq!(identifier.to_string(), "Hello World");

        let identifier = Arc::new("Hello World".to_owned());
        let identifier = TaskIdentifier::from(identifier);
        assert_eq!(identifier.to_string(), "Hello World");
    }
}
