use std::fmt::{Error, Formatter};
use std::cmp::Ordering;

/// Unique identifier for running actors.
#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct ActorPath(Vec<String>);

impl ActorPath {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn root(&self) -> Self {
        if self.0.len() == 1 {
            self.clone()
        } else if !self.0.is_empty() {
            ActorPath(self.0.iter().take(1).cloned().collect())
        } else {
            ActorPath(Vec::new())
        }
    }

    pub fn parent(&self) -> Self {
        if self.0.len() > 1 {
            let mut tokens = self.0.clone();
            tokens.truncate(tokens.len() - 1);
            ActorPath(tokens)
        } else {
            ActorPath(Vec::new())
        }
    }

    pub fn key(&self) -> String {
        self.0.last().cloned().unwrap_or_else(|| "".to_string())
    }

    pub fn level(&self) -> usize {
        self.0.len()
    }

    pub fn at_level(&self, level: usize) -> Self {
        if level < 1 || level >= self.level() {
            self.clone()
        } else if self.is_top_level() {
            self.root()
        } else if level == self.level() - 1 {
            self.parent()
        } else {
            let mut tokens = self.0.clone();
            tokens.truncate(level);
            ActorPath(tokens)
        }
    }

    pub fn is_ancestor_of(&self, other: &ActorPath) -> bool {
        let me = format!("{}/", self);
        other.to_string().as_str().starts_with(me.as_str())
    }

    pub fn is_descendant_of(&self, other: &ActorPath) -> bool {
        let me = self.to_string();
        me.as_str().starts_with(format!("{}/", other).as_str())
    }

    pub fn is_parent_of(&self, other: &ActorPath) -> bool {
        *self == other.parent()
    }

    pub fn is_child_of(&self, other: &ActorPath) -> bool {
        self.parent() == *other
    }

    pub fn is_top_level(&self) -> bool {
        self.0.len() == 1
    }
}

impl From<&str> for ActorPath {
    fn from(str: &str) -> Self {
        let tokens: Vec<String> = str.split('/')
            .filter(|x| !x.trim().is_empty())
            .map(|s| s.to_string() )
            .collect();

        ActorPath(tokens)
    }
}

impl From<String> for ActorPath {
    fn from(string: String) -> Self {
        ActorPath::from(string.as_str())
    }
}

impl From<&String> for ActorPath {
    fn from(string: &String) -> Self {
        ActorPath::from(string.as_str())
    }
}

impl std::ops::Div<&str> for ActorPath {
    type Output = ActorPath;

    fn div(self, rhs: &str) -> Self::Output {
        let mut keys = self.0;
        keys.push(rhs.to_string());
        ActorPath(keys)
    }
}

impl std::fmt::Display for ActorPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self.level().cmp(&1) {
            Ordering::Less => write!(f, "/"),
            Ordering::Equal => write!(f, "/{}", self.0[0]),
            Ordering::Greater => write!(f, "/{}", self.0.join("/"))
        }
    }
}

impl std::fmt::Debug for ActorPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self.level().cmp(&1) {
            Ordering::Less => write!(f, "/"),
            Ordering::Equal => write!(f, "/{}", self.0[0]),
            Ordering::Greater => write!(f, "/{}", self.0.join("/"))
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_empty_string() {
        let path = ActorPath::from("");
        assert_eq!(path.0, Vec::<String>::new());
    }

    #[test]
    fn parse_single_root() {
        let path = ActorPath::from("/acme");
        println!("{:?}", path);
        assert_eq!(path.0, vec!["acme"]);
    }

    #[test]
    fn parse_two_deep() {
        let path = ActorPath::from("/acme/building");
        println!("{:?}", path);
        assert_eq!(path.0, vec!["acme", "building"]);
    }

    #[test]
    fn parse_three_deep() {
        let path = ActorPath::from("/acme/building/room");
        println!("{:?}", path);
        assert_eq!(path.0, vec!["acme", "building", "room"]);
    }

    #[test]
    fn parse_levels() {
        let path = ActorPath::from("/acme/building/room/sensor");
        println!("{:?}", path);
        assert_eq!(path.level(), 4);
    }

    #[test]
    fn test_get_key() {
        let path = ActorPath::from("/acme/building/room/sensor");
        println!("{:?}", path);
        assert_eq!(path.key(), "sensor".to_string());
    }

    #[test]
    fn parse_get_parent() {
        let path = ActorPath::from("/acme/building/room/sensor").parent();
        println!("{:?}", path);
        assert_eq!(path.parent().0, vec!["acme", "building"]);
    }

    #[test]
    fn parse_to_string() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let string = path.to_string();
        println!("{:?}", string);
        assert_eq!(string, "/acme/building/room/sensor");
    }

    #[test]
    fn parse_root_at_root() {
        let path = ActorPath::from("/acme");
        let string = path.root().to_string();
        println!("{:?}", string);
        assert_eq!(string, "/acme");
    }

    #[test]
    fn parse_parent_at_root() {
        let path = ActorPath::from("/acme");
        let string = path.parent().to_string();
        println!("{:?}", string);
        assert_eq!(string, "/");
    }

    #[test]
    fn parse_root_to_string() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let string = path.root().to_string();
        println!("{:?}", string);
        assert_eq!(string, "/acme");
    }

    #[test]
    fn test_if_empty() {
        let path = ActorPath::from("/");
        assert!(path.is_empty());
        let not_empty = ActorPath::from("/not_empty");
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_if_parent_child() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let parent = path.parent();
        assert!(parent.is_parent_of(&path));
        assert!(path.is_child_of(&parent));
    }

    #[test]
    fn test_if_descendant() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let parent = path.parent();
        assert!(path.is_descendant_of(&parent));
        assert!(!path.is_descendant_of(&path));
    }

    #[test]
    fn test_if_ancestor() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let parent = path.parent();
        assert!(parent.is_ancestor_of(&path));
        assert!(!path.is_ancestor_of(&path));
    }

    #[test]
    fn test_if_ancestor_descendant() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let root = path.root();
        assert!(root.is_ancestor_of(&path));
        assert!(path.is_descendant_of(&root));
    }

    #[test]
    fn test_if_root() {
        let path = ActorPath::from("/acme/building/room/sensor");
        let root = path.root();
        println!("{:?}", path);
        println!("{:?}", root);
        assert!(root.is_top_level());
        assert!(!path.is_top_level());
    }

    #[test]
    fn test_at_level() {
        let path = ActorPath::from("/acme/building/room/sensor");
        assert_eq!(path.at_level(0), path);
        assert_eq!(path.at_level(1), path.root());
        assert_eq!(path.at_level(2), ActorPath::from("/acme/building"));
        assert_eq!(path.at_level(3), path.parent());
        assert_eq!(path.at_level(4), path);
        assert_eq!(path.at_level(5), path);
    }

    #[test]
    fn test_add_path() {
        let path = ActorPath::from("/acme");
        let child = path.clone() / "child";
        println!("{}", &child);
        assert!(path.is_parent_of(&child))
    }
}
