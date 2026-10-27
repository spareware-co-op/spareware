//! JSON Schema builders for command parameters, a small validator, and a typed reader for parameters.

use crate::error::{ApiError, ApiResult};
use serde_json::{Map, Value, json};

/// A text parameter.
pub fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

/// A number parameter.
pub fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

/// A whole-number parameter.
pub fn integer(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}

/// A true or false parameter.
pub fn boolean(description: &str) -> Value {
    json!({ "type": "boolean", "description": description })
}

/// A list parameter whose items follow the given schema.
pub fn array(items: Value, description: &str) -> Value {
    json!({ "type": "array", "items": items, "description": description })
}

/// An object parameter with any keys.
pub fn free_object(description: &str) -> Value {
    json!({ "type": "object", "description": description })
}

/// A parameter of any JSON type.
pub fn any(description: &str) -> Value {
    json!({ "description": description })
}

/// A text parameter that must be one of the listed values.
pub fn choice(values: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": values, "description": description })
}

/// A position: [latitude, longitude] in degrees.
pub fn latlng(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2, "description": description })
}

/// A list of text values.
pub fn strings(description: &str) -> Value {
    array(json!({ "type": "string" }), description)
}

/// The schema of a command's parameters: an object with these properties, the required ones named, and no other keys.
pub fn params(props: Vec<(&str, Value)>, required: &[&str]) -> Value {
    let mut p = Map::new();
    for (k, v) in props {
        p.insert(k.to_string(), v);
    }
    json!({ "type": "object", "properties": p, "required": required, "additionalProperties": false })
}

/// The schema of a command that takes no parameters.
pub fn none() -> Value {
    params(vec![], &[])
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "text",
        Value::Array(_) => "a list",
        Value::Object(_) => "an object",
    }
}

/// Checks a value against a schema made with the builders above: types, enums, required keys, unknown keys (when the schema forbids them) and list items. Null counts as absent for an optional key.
pub fn validate(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let at = if path.is_empty() {
        "the parameters".to_string()
    } else {
        format!("`{path}`")
    };
    if let Some(t) = schema.get("type").and_then(Value::as_str) {
        let ok = match t {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_f64().is_some_and(|x| x.fract() == 0.0),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            _ => true,
        };
        if !ok {
            let want = match t {
                "string" => "text",
                "number" => "a number",
                "integer" => "a whole number",
                "boolean" => "true or false",
                "array" => "a list",
                _ => "an object",
            };
            return Err(format!("{at} must be {want}, not {}", type_name(value)));
        }
    }
    if let Some(e) = schema.get("enum").and_then(Value::as_array)
        && !e.contains(value)
    {
        let list: Vec<String> = e
            .iter()
            .map(|x| x.as_str().map_or(x.to_string(), str::to_string))
            .collect();
        return Err(format!("{at} must be one of: {}", list.join(", ")));
    }
    if let (Some(items), Some(arr)) = (schema.get("items"), value.as_array()) {
        if let Some(n) = schema.get("minItems").and_then(Value::as_u64)
            && (arr.len() as u64) < n
        {
            return Err(format!("{at} needs at least {n} items"));
        }
        if let Some(n) = schema.get("maxItems").and_then(Value::as_u64)
            && (arr.len() as u64) > n
        {
            return Err(format!("{at} takes at most {n} items"));
        }
        for (i, x) in arr.iter().enumerate() {
            let p = if path.is_empty() {
                format!("[{i}]")
            } else {
                format!("{path}[{i}]")
            };
            validate(items, x, &p)?;
        }
    }
    if let Some(obj) = value.as_object()
        && let Some(props) = schema.get("properties").and_then(Value::as_object)
    {
        if let Some(req) = schema.get("required").and_then(Value::as_array) {
            for r in req.iter().filter_map(Value::as_str) {
                if obj.get(r).is_none_or(Value::is_null) {
                    let k = if path.is_empty() {
                        r.to_string()
                    } else {
                        format!("{path}.{r}")
                    };
                    return Err(format!("missing parameter `{k}`"));
                }
            }
        }
        if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            for k in obj.keys() {
                if !props.contains_key(k) {
                    let mut allowed: Vec<&str> = props.keys().map(String::as_str).collect();
                    allowed.sort_unstable();
                    let name = if path.is_empty() {
                        k.to_string()
                    } else {
                        format!("{path}.{k}")
                    };
                    return Err(format!(
                        "unknown parameter `{name}`; this command takes: {}",
                        if allowed.is_empty() {
                            "no parameters".to_string()
                        } else {
                            allowed.join(", ")
                        }
                    ));
                }
            }
        }
        for (k, v) in obj {
            if v.is_null() {
                continue;
            }
            if let Some(s) = props.get(k) {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                validate(s, v, &p)?;
            }
        }
    }
    Ok(())
}

/// A typed reader over a command's parameters (a JSON object).
#[derive(Clone, Copy)]
pub struct Params<'a> {
    v: &'a Value,
}

fn missing(k: &str) -> ApiError {
    ApiError::bad_params(format!("missing parameter `{k}`"))
}

fn wrong(k: &str, want: &str, got: &Value) -> ApiError {
    ApiError::bad_params(format!("`{k}` must be {want}, not {}", type_name(got)))
}

impl<'a> Params<'a> {
    /// Reads parameters from a JSON value (null reads as no parameters).
    pub fn new(v: &'a Value) -> Params<'a> {
        static EMPTY: Value = Value::Null;
        Params {
            v: if v.is_object() { v } else { &EMPTY },
        }
    }
    /// The raw value of a key, when given and not null.
    pub fn get(&self, k: &str) -> Option<&'a Value> {
        self.v.get(k).filter(|x| !x.is_null())
    }
    /// True when the key is given and not null.
    pub fn has(&self, k: &str) -> bool {
        self.get(k).is_some()
    }
    /// The whole parameters object.
    pub fn value(&self) -> &'a Value {
        self.v
    }
    /// A text parameter that must be given.
    pub fn str(&self, k: &str) -> ApiResult<&'a str> {
        self.opt_str(k)?.ok_or_else(|| missing(k))
    }
    /// A text parameter that may be left out.
    pub fn opt_str(&self, k: &str) -> ApiResult<Option<&'a str>> {
        match self.get(k) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(o) => Err(wrong(k, "text", o)),
        }
    }
    /// A number parameter that must be given.
    pub fn f64(&self, k: &str) -> ApiResult<f64> {
        self.opt_f64(k)?.ok_or_else(|| missing(k))
    }
    /// A number parameter that may be left out.
    pub fn opt_f64(&self, k: &str) -> ApiResult<Option<f64>> {
        match self.get(k) {
            None => Ok(None),
            Some(Value::Number(n)) => Ok(n.as_f64()),
            Some(o) => Err(wrong(k, "a number", o)),
        }
    }
    /// A true or false parameter that may be left out.
    pub fn opt_bool(&self, k: &str) -> ApiResult<Option<bool>> {
        match self.get(k) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(o) => Err(wrong(k, "true or false", o)),
        }
    }
    /// A true or false parameter, false when left out.
    pub fn flag(&self, k: &str) -> ApiResult<bool> {
        Ok(self.opt_bool(k)?.unwrap_or(false))
    }
    /// A list parameter that may be left out.
    pub fn opt_array(&self, k: &str) -> ApiResult<Option<&'a Vec<Value>>> {
        match self.get(k) {
            None => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(o) => Err(wrong(k, "a list", o)),
        }
    }
    /// A list parameter that must be given.
    pub fn array(&self, k: &str) -> ApiResult<&'a Vec<Value>> {
        self.opt_array(k)?.ok_or_else(|| missing(k))
    }
    /// An object parameter that may be left out.
    pub fn opt_object(&self, k: &str) -> ApiResult<Option<&'a Map<String, Value>>> {
        match self.get(k) {
            None => Ok(None),
            Some(Value::Object(m)) => Ok(Some(m)),
            Some(o) => Err(wrong(k, "an object", o)),
        }
    }
    /// An object parameter that must be given.
    pub fn object(&self, k: &str) -> ApiResult<&'a Map<String, Value>> {
        self.opt_object(k)?.ok_or_else(|| missing(k))
    }
    /// A list of text values that may be left out.
    pub fn opt_strings(&self, k: &str) -> ApiResult<Option<Vec<String>>> {
        match self.opt_array(k)? {
            None => Ok(None),
            Some(a) => a
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| wrong(k, "a list of text", x))
                })
                .collect::<ApiResult<Vec<_>>>()
                .map(Some),
        }
    }
    /// A position [latitude, longitude] that may be left out.
    pub fn opt_latlng(&self, k: &str) -> ApiResult<Option<[f64; 2]>> {
        match self.get(k) {
            None => Ok(None),
            Some(v) => latlng_of(v, k).map(Some),
        }
    }
    /// A position [latitude, longitude] that must be given.
    pub fn latlng(&self, k: &str) -> ApiResult<[f64; 2]> {
        self.opt_latlng(k)?.ok_or_else(|| missing(k))
    }
}

/// A [latitude, longitude] pair from a JSON list, with both numbers finite.
pub fn latlng_of(v: &Value, name: &str) -> ApiResult<[f64; 2]> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| {
        ApiError::bad_params(format!("`{name}` must be a [latitude, longitude] pair"))
    })?;
    let (la, ln) = (a[0].as_f64(), a[1].as_f64());
    match (la, ln) {
        (Some(la), Some(ln)) if la.is_finite() && ln.is_finite() => Ok([la, ln]),
        _ => Err(ApiError::bad_params(format!(
            "`{name}` must be two numbers [latitude, longitude]"
        ))),
    }
}
