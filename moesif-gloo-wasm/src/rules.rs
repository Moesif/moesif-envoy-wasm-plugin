use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer, Visitor, MapAccess};
use std::{collections::HashMap, fmt};

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceRulesResponse {
    pub rules: Vec<GovernanceRule>,
    pub e_tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GovernanceRule {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub block: bool,
    pub regex_config: Vec<RegexConditionsAnd>,
    pub response: ResponseOverrides,
    pub variables: Option<Vec<Variable>>,
    pub org_id: String,
    pub app_id: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegexConditionsAnd {
    pub conditions: Vec<RegexCondition>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegexCondition {
    pub path: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseOverrides {
    pub body: Option<BodyTemplate>,
    pub headers: HashMap<String, String>,
    pub status: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct BodyTemplate(pub String);
struct BodyTemplateVisitor;

impl<'de> Visitor<'de> for BodyTemplateVisitor {
    type Value = BodyTemplate;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON object as a string")
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<BodyTemplate, V::Error>
    where
        V: MapAccess<'de>,
    {
        let mut body = serde_json::Map::new();
        while let Some((key, value)) = visitor.next_entry()? {
            body.insert(key, value);
        }
        let body_string = serde_json::to_string(&body).map_err(de::Error::custom)?;
        Ok(BodyTemplate(body_string))
    }
}

impl<'de> Deserialize<'de> for BodyTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(BodyTemplateVisitor)
    }
}

pub fn template(t: &str, vars: &HashMap<String, String>) -> String {
    let mut s = t.to_owned();
    for (name, value) in vars {
        s = s.replace(&format!("{{{{{}}}}}", name), value);
    }
    s
}
