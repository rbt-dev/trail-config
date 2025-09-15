# Trail Config

Simple [Rust](https://www.rust-lang.org/) library to help with reading (and formatting) values from config files.\
Supports YAML format (uses [serde_yaml_bw](https://github.com/bourumir-wyngs/serde-yaml-bw) library).

## Examples

### Sample *config.yaml* file
```yaml
app:
  port: 1000
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
```

### Default configuration
```rust
let config = Config::default(); // loads config.yaml file

let port = config.get("app/port").unwrap(); // returns serde_yaml::value::Value

let port = config.str("app/port");
assert_eq!("1000", port);

let redis = config.get("db/redis"); // returns serde_yaml::value::Value (in this case Mapping)

let redis = config.str("db/redis");
assert_eq!("", redis);

let expiry = config.str("db/redis/key_expiry");
assert_eq!("3600", expiry);

let redis = config.fmt("{}:{}", "db/redis/server+port");
assert_eq!("127.0.0.1:6379", redis);

let conn = config.fmt("Driver={{{}}};Server={};Database={};Uid={};Pwd={};", "db/sql/driver+server+database+username+password");
assert_eq!("Driver={SQL Server};Server=127.0.0.1;Database=my_db;Uid=user;Pwd=Pa$$w0rd!;", conn);
```

### With custom separator
```rust
let config = Config::new("config.yaml", "::", None).unwrap(); 

let port = config.str("app::port");
assert_eq!("1000", port);
```

### With environment variable
```rust
let config = Config::new("config.{env}.yaml", "/", Some("dev")).unwrap(); // loads config.dev.yaml
assert_eq!("dev", config.environment().unwrap());
```


## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details