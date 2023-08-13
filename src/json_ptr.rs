use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// See [RFC 6901](https://datatracker.ietf.org/doc/html/rfc6901)
#[derive(Debug, Default)]
pub struct JsonPtr(String);

impl JsonPtr {
    /// Returns the escaped path
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create a new [`JsonPtr`] from a list of segments.
    /// Each segment will be escaped and joined.
    pub fn new<I, S>(path: I) -> JsonPtr
    where
        I: IntoIterator<Item = S> + Copy,
        S: AsRef<str>,
    {
        // https://datatracker.ietf.org/doc/html/rfc6901#section-4
        fn escape(segment: &str) -> String {
            segment.replace('~', "~0").replace('/', "~1")
        }

        let min_len = path
            .into_iter()
            .fold(0, |acc, next| acc + next.as_ref().len() + 1);
        let mut ptr = String::with_capacity(min_len);

        path.into_iter().for_each(|part| {
            ptr.push('/');
            ptr.push_str(&escape(part.as_ref()));
        });

        JsonPtr(ptr)
    }

    /// Deserialize [`JsonPtr`] from a [`Vec<String>`]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<JsonPtr, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = Vec::<String>::deserialize(deserializer)?;
        Ok(JsonPtr::new(&raw))
    }

    /// Walks the pointer in `val` and returns the value as `f64`.
    pub fn get_f64(&self, val: &Value) -> Option<f64> {
        val.pointer(self.as_str())?.as_f64()
    }
}

#[cfg(test)]
mod tests {
    use crate::json_ptr::JsonPtr;
    use serde::Deserialize;

    #[test]
    fn path() {
        let path = &["~e~ngine~s~", "/R/ender/3D/", "busy~"];
        let exp = "/~0e~0ngine~0s~0/~1R~1ender~13D~1/busy~0";
        assert_eq!(exp, JsonPtr::new(path).as_str());
    }

    #[test]
    fn empty() {
        let path: &[&str] = &[];
        assert_eq!("", JsonPtr::new(path).as_str());
    }

    #[test]
    fn deserialize() {
        #[derive(Debug, Deserialize)]
        struct Struct {
            name: String,
            #[serde(deserialize_with = "JsonPtr::deserialize")]
            path: JsonPtr,
        }

        let json = r#"{ "name": "hi", "path": ["a", "/", "~", "b"] }"#;
        let result = serde_json::from_str::<Struct>(json).unwrap();

        assert_eq!(result.name, "hi");
        assert_eq!(result.path.as_str(), "/a/~1/~0/b");
    }
}
