use std::{collections::HashMap, fmt};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use crate::event::{RequestInfo, ResponseInfo};

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
    pub applied_to: String,
    pub applied_to_unidentified: bool,
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

pub struct RuleTemplate {
    rule: GovernanceRule,
    values: HashMap<String, String>,
}

impl RuleTemplate {
    fn template_override(&self) -> TemplatedOverrideValues {
        let mut headers = HashMap::new();
        for (k, v) in &self.rule.response.headers {
            headers.insert(k.clone(), template(v, &self.values));
        }
        let body = self.rule.response.body.as_ref().map(|b| template(&b.0, &self.values));
        TemplatedOverrideValues {
            block: self.rule.block,
            headers,
            status: self.rule.response.status,
            body: body.map(|b| b.into_bytes()),
        }
    }
}

pub struct TemplatedOverrideValues {
    block: bool,
    headers: HashMap<String, String>,
    status: i32,
    body: Option<Vec<u8>>,
}

pub struct ResponseOverride {
    override_values: TemplatedOverrideValues,
    response: ResponseInfo,
    wrote_headers: bool,
    wrote_body: bool,
}

impl ResponseOverride {
    fn new(response: ResponseInfo, templates: Vec<RuleTemplate>) -> Self {
        let mut override_values = TemplatedOverrideValues {
            block: false,
            headers: HashMap::new(),
            status: 0,
            body: None,
        };
        for template in templates {
            let t = template.template_override();
            override_values.block |= t.block;
            override_values.status = t.status;
            for (k, v) in t.headers {
                override_values.headers.insert(k, v);
            }
            if let Some(body) = t.body {
                override_values.body = Some(body);
            }
        }
        Self {
            override_values,
            response,
            wrote_headers: false,
            wrote_body: false,
        }
    }
}

fn check_regex(rule: &GovernanceRule, req: &RequestInfo) -> bool {
    if rule.regex_config.is_empty() {
        return true;
    }
    for regex_and in &rule.regex_config {
        let and_value = regex_and.conditions.iter().all(|c| {
            let s = request_path_lookup(&req, &c.path);
            let re = Regex::new(&c.value);
            match re {
                Ok(re) => re.is_match(&s),
                Err(_) => {
                    eprintln!("Governance rule regex error: org-app={}-{} rule.id={} rule.name={} path={} regex={}", rule.org_id, rule.app_id, rule.id, rule.name, c.path, c.value);
                    false
                }
            }
        });
        if and_value {
            return true;
        }
    }
    false
}

fn request_path_lookup(req: &RequestInfo, path: &str) -> String {
    match path {
        "request.uri" => req.uri.clone(),
        "request.verb" => req.verb.clone(),
        // Add more path cases based on your needs
        _ => "".into(),
    }
}

