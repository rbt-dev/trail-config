use std::{collections::HashMap, error::Error, fs};
use serde_yaml::{Value, from_str};
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

    pub fn environment(self) -> Option<String> {
        self.environment
    }

    pub fn get(&self, path: &str) -> Option<Value> {
        let mut content = &self.content.clone();
        let parts = path.split(&self.separator).collect::<Vec<&str>>();
    
        for item in parts.iter() {
            match content.get(item) {
                Some(v) => { content = v; },
                None => return None
            }
        }
        
        Some(content.clone())
    }

    pub fn str(&self, path: &str) -> String {
        let mut content = &self.content.clone();
        let parts = path.split(&self.separator).collect::<Vec<&str>>();
    
        for item in parts.iter() {
            match content.get(item) {
                Some(v) => { content = v; },
                None => return String::new()
            }
        }
        
        Self::to_string(content)
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
            Value::String(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            Value::Bool(v) => v.to_string(),
            _ => String::new()
        }
    }
}