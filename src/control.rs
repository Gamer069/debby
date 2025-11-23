use std::collections::HashMap;
use sqlite3::{Connection, Error, State};

use serde::Deserialize;

macro_rules! fielded_struct {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $(
                $fvis:vis $fname:ident : $ftype:ty
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis struct $name {
            $(
                $fvis $fname: $ftype
            ),*
        }

        impl $name {
            pub fn sql_fields() -> String {
                vec![$(stringify!($fname)),*].join(", ")
            }

            pub fn fields() -> Vec<String> {
                vec![$(stringify!($fname).to_string()),*]
            }

            pub fn populate_sql(&self) -> (String, String) {
                let mut cols = Vec::new();
                let mut vals = Vec::new();

                $(
                    cols.push(stringify!($fname).to_string());
                    vals.push(format_field(&self.$fname));
                )*

                (cols.join(", "), vals.join(", "))
            }

            pub fn field(&self, field_name: &str) -> Option<String> {
                match field_name {
                    $(
                        stringify!($fname) => Some(format_field(&self.$fname)),
                    )*
                    _ => None,
                }
            }
        }
    };
}

#[derive(Clone, Debug)]
pub struct ControlWithData {
    pub ctrl: Control,
    pub installed: String,
}

impl ControlWithData {
    pub fn from_db(conn: &Connection, package_name: &str, version: &str) -> Result<Self, Error> {
        let query = format!(
            "SELECT {} FROM debs WHERE package = ? AND version = ?",
            Control::sql_fields() + ", installed"
        );

        let mut stmt = conn.prepare(&query)?;
        stmt.bind(1, package_name)?;
        stmt.bind(2, version)?;

        if stmt.next()? == State::Row {
            let mut map = HashMap::new();

            for i in 0..stmt.columns() {
                let column_name = stmt.column_names().unwrap()[i].clone();
                if let Ok(value) = stmt.read::<String>(i) {
                    map.insert(column_name.to_string(), value);
                }
            }

            let mut modified_map = map.clone();
            modified_map.remove("installed");
            modified_map.remove("id");

            let installed = match map.get("installed") {
                Some(installed) => installed.to_string(),
                None => return Err(sqlite3::Error{code: None, message: Some("Could not find 'installed' field".to_string())})
            };

            let ctrl = match from_map(modified_map) {
                Ok(ctrl) => ctrl,
                Err(e) => return Err(sqlite3::Error{code: None, message: Some(format!("Failed to parse control file: {}", e))})
            };

            let cwd = Self { ctrl, installed };

            Ok(cwd)
        } else {
            Err(sqlite3::Error { code: None, message: Some(".deb is not installed".to_string()) })
        }
    }
}

// Helper trait to handle formatting of different types
trait SqlFormat {
    fn format_sql(&self) -> String;
}

// Implement for common types
impl<T: std::fmt::Display> SqlFormat for Option<T> {
    fn format_sql(&self) -> String {
        match self {
            Some(val) => format!("'{}'", val.to_string().replace("'", "''")),
            None => "NULL".to_string(),
        }
    }
}

impl SqlFormat for String {
    fn format_sql(&self) -> String {
        format!("'{}'", self.replace("'", "''"))
    }
}

impl SqlFormat for &str {
    fn format_sql(&self) -> String {
        format!("'{}'", self.replace("'", "''"))
    }
}

impl SqlFormat for i32 {
    fn format_sql(&self) -> String {
        self.to_string()
    }
}

impl SqlFormat for i64 {
    fn format_sql(&self) -> String {
        self.to_string()
    }
}

fn format_field<T: SqlFormat>(field: &T) -> String {
    field.format_sql()
}

fielded_struct! {
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    pub struct Control {
        pub package: String,
		pub version: String,
		pub architecture: String,
		pub maintainer: String,
		pub description: String,

		pub depends: Option<String>,
		pub pre_depends: Option<String>,
		pub provides: Option<String>,
		pub section: Option<String>,
		pub priority: Option<String>,
		pub installed_size: Option<String>,
		pub recommends: Option<String>,
		pub suggests: Option<String>,
		pub enhances: Option<String>,
		pub breaks: Option<String>,
		pub conflicts: Option<String>,
		pub replaces: Option<String>,
		pub bugs: Option<String>,
		pub license: Option<String>,
		pub homepage: Option<String>,
		pub origin: Option<String>
    }
}

pub fn parse_control(control: String) -> Result<Control, serde_json::Error> {
    let lines = control.lines().collect::<Vec<_>>();
    let mut kvs: HashMap<String, String> = HashMap::new();
    let mut current_key: Option<String> = None;

    for line in lines {
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation line - append to current value
            if let Some(key) = &current_key {
                if let Some(val) = kvs.get_mut(key) {
                    val.push('\n');
                    val.push_str(line.trim());
                }
            }
        } else if let Some((key, value)) = line.split_once(':') {
            // New key-value pair
            let key = key.trim().to_string();
            current_key = Some(key.clone());
            kvs.insert(key, value.trim().to_string());
        }
    }

    from_map(kvs)
}

pub fn from_map(map: HashMap<String, String>) -> Result<Control, serde_json::Error> {
    serde_json::from_value(serde_json::Value::Object(
        map.into_iter()
            .map(|(k, v)| (k.to_lowercase(), v.into()))
            .collect()
    ))
}
