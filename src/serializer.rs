pub trait SerDe {
    fn to_string<T: serde::ser::Serialize>(&self, data: &T) -> anyhow::Result<String>;
    #[allow(clippy::wrong_self_convention)]
    fn from_str<T: serde::de::DeserializeOwned>(&self, data: &str) -> anyhow::Result<T>;
    fn ending(&self) -> String;
}

#[derive(Clone)]
pub struct Json {}

impl SerDe for Json {
    fn to_string<T: serde::ser::Serialize>(&self, data: &T) -> anyhow::Result<String> {
        serde_json::to_string_pretty(data).map_err(|e| anyhow::anyhow!(e))
    }

    fn from_str<T>(&self, data: &str) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_str(data).map_err(|e| anyhow::anyhow!(e))
    }

    fn ending(&self) -> String {
        "json".to_owned()
    }
}

#[derive(Clone)]
pub struct Toml {}

impl SerDe for Toml {
    fn to_string<T: serde::ser::Serialize>(&self, data: &T) -> anyhow::Result<String> {
        toml::to_string(data).map_err(|e| anyhow::anyhow!(e))
    }

    fn from_str<T>(&self, data: &str) -> anyhow::Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        toml::from_str(data).map_err(|e| anyhow::anyhow!(e))
    }

    fn ending(&self) -> String {
        "toml".to_owned()
    }
}
