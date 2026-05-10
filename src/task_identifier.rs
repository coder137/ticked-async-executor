use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TaskId {
    pub task_id: u64,
    pub user_id: UserId,
}

/// Cheaply clonable UserId
#[derive(Debug, Clone)]
pub enum UserId {
    None,
    Literal(&'static str),
    Arc(Arc<String>),
}

impl From<()> for UserId {
    fn from(_value: ()) -> Self {
        Self::None
    }
}

impl From<&'static str> for UserId {
    fn from(value: &'static str) -> Self {
        Self::Literal(value)
    }
}

impl From<String> for UserId {
    fn from(value: String) -> Self {
        Self::Arc(Arc::new(value))
    }
}

impl From<Arc<String>> for UserId {
    fn from(value: Arc<String>) -> Self {
        Self::Arc(value.clone())
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserId::None => write!(f, "[None]"),
            UserId::Literal(data) => write!(f, "{data}"),
            UserId::Arc(data) => write!(f, "{data}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let identifier = UserId::from(());
        assert_eq!(identifier.to_string(), "[None]");

        let identifier = UserId::from("Hello World");
        assert_eq!(identifier.to_string(), "Hello World");

        let identifier = "Hello World".to_owned();
        let identifier = UserId::from(identifier);
        assert_eq!(identifier.to_string(), "Hello World");

        let identifier = Arc::new("Hello World".to_owned());
        let identifier = UserId::from(identifier);
        assert_eq!(identifier.to_string(), "Hello World");
    }
}
