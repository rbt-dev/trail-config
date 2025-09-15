use std::{collections::HashMap, error::Error, fs};
use serde_yaml_bw::{Value, from_str};
use strfmt::strfmt;

#[derive(Debug, Clone)]
pub struct Config {
    content: Value,
    filename: String,
    separator: String,
    environment: Option<String>
}

impl Default for Config {
    fn default() -> Self {
        Self::new("config.yaml", "/", None).unwrap()
    }
}

impl Config {
    pub fn new(filename: &str, sep: &str, env: Option<&str>) -> Result<Config, Box<dyn Error>> {
        let (file, env) = Self::get_file(filename, env);

        match Self::load(&file) {
            Ok(yaml) => Ok(Config {
                content: yaml,
                filename: file,
                separator: sep.to_string(),
                environment: env
            }),
            Err(e) => Err(e)
        }
    }

    pub fn environment(&self) -> Option<&str> {
        match &self.environment {
            Some(v) => Some(v),
            None => None
        }
    }

    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    pub fn get(&self, path: &str) -> Option<Value> {
        Self::get_leaf(&self.content, path, &self.separator)
    }

    pub fn str(&self, path: &str) -> String {
        let content = Self::get_leaf(&self.content, path, &self.separator);

        match content {
            Some(v) => Self::to_string(&v),
            None => String::new()
        }
    }

    pub fn list(&self, path: &str) -> Vec<String> {
        let content = Self::get_leaf(&self.content, path, &self.separator);
        
        match content {
            Some(v) => Self::to_list(&v),
            None => vec![]
        }
    }

    pub fn fmt(&self, format: &str, path: &str) -> String {
        let mut content = &self.content.clone();
        let mut parts = path.split(&self.separator).collect::<Vec<&str>>();
        let last = parts.pop();
    
        for item in parts.iter() {
            match content.get(item) {
                Some(v) => { content = v; },
                None => return String::new()
            }
        }

        match last {
            Some(v) => {
                let attributes = v.split('+').collect::<Vec<&str>>();
                let mut fmt = format.to_string();
                let mut vars = HashMap::new();

                for item in attributes.iter() {
                    match content.get(item) {
                        Some(v) => {
                            fmt = fmt.replacen("{}", &format!("{{{}}}", item), 1);
                            vars.insert(item.to_string(), Self::to_string(v));
                        },
                        None => return String::new()
                    }
                }

                return match strfmt(&fmt, &vars) {
                    Ok(r) => r,
                    Err(_) => String::new()
                };
            },
            None => String::new()
        }
    }

    pub fn load_yaml(yaml: &str, sep: &str) -> Result<Config, Box<dyn Error>> {
        let parsed = from_str(&yaml)?;

        Ok(Config {
            content: parsed,
            filename: String::new(),
            separator: sep.to_string(),
            environment: None
        })
    }

    fn get_leaf(mut content: &Value, path: &str, separator: &str) -> Option<Value> {
        let parts = path.split(separator).collect::<Vec<&str>>();
    
        for item in parts.iter() {
            match content.get(item) {
                Some(v) => { content = v; },
                None => return None
            }
        }

        return Some(content.clone());
    }

    fn get_file(filename: &str, env: Option<&str>) -> (String, Option<String>) {
        match env {
            Some(v) => {
                let mut vars = HashMap::new();
                vars.insert(String::from("env"), v);
                (strfmt(filename, &vars).unwrap(), Some(v.to_string()))
            },
            None => (String::from(filename), None)
        }
    }

    fn load(filename: &str) -> Result<Value, Box<dyn Error>> {
        let yaml = fs::read_to_string(filename)?;
        let parsed = from_str(&yaml)?;
        
        Ok(parsed)
    }
    
    fn to_string(value: &Value) -> String {
        match value {
            Value::String(v, _) => v.to_string(),
            Value::Number(v, _) => v.to_string(),
            Value::Bool(v, _) => v.to_string(),
            _ => String::new()
        }
    }

    fn to_list(value: &Value) -> Vec<String> {
        match value {
            Value::Sequence(v) => v.iter().map(Self::to_string).collect::<Vec<String>>(),
            _ => vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{from_str, Config, Value};
    use serde_yaml_bw::Number;

    const YAML: &str = "
db:
    redis:
        server: 127.0.0.1
        port: 6379
        key_expiry: 3600
    sql:
        driver: SQL Server
        server: 127.0.0.1
        database: my_db
        username: user
        password: Pa$$w0rd!
sources:
    - one
    - two
    - three
";

    #[test]
    fn fmt_test()  {
        let parsed: Config = Config::load_yaml(YAML, "/").unwrap();
        let formatted = parsed.fmt("{}:{}", "db/sql/database+username");

        assert_eq!(formatted, String::from("my_db:user"));
    }

    #[test]
    fn get_leaf_test()  {
        let parsed: Value = from_str(YAML).unwrap();
        let value1 = Config::get_leaf(&parsed, "db/redis/port", "/");
        let value2 = Config::get_leaf(&parsed, "db/redis/username", "/");
        
        assert_eq!(value1, Some(Value::Number(Number::from(6379), None)));
        assert_eq!(value2, None);
    }

    #[test]
    fn get_file_test() {
        let (file, env) = Config::get_file("config_{env}.yaml", Some("dev"));

        assert_eq!(env,  Some(String::from("dev")));
        assert_eq!(file, "config_dev.yaml");
    }

    #[test]
    fn to_string_test() {
        let parsed: Value = from_str(YAML).unwrap();
        let value = Config::get_leaf(&parsed, "db/redis/port", "/").unwrap();
        let str_value = Config::to_string(&value);

        assert_eq!(str_value, "6379");
    }

    #[test]
    fn to_list_test()  {
        let parsed: Value = from_str(YAML).unwrap();
        let value = Config::get_leaf(&parsed, "sources", "/").unwrap();
        let list = Config::to_list(&value);

        let mut vec = Vec::new();        
        vec.push(String::from("one"));
        vec.push(String::from("two"));
        vec.push(String::from("three"));

        assert_eq!(list, vec);
    }
}