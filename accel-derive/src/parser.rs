use failure::*;
use nvptx::manifest::Crate;
use quote::quote;
use std::fs;

pub struct Attributes {
    crates: Vec<Crate>,
}

impl Attributes {
    pub fn parse(attrs: &[syn::Attribute]) -> Self {
        Self {
            crates: attrs.iter().map(|attr| parse_crate(attr)).collect(),
        }
    }

    pub fn get_crates(&self) -> &[Crate] {
        &self.crates
    }

    /// Create a nvptx compiler-driver
    pub fn create_driver(&self) -> nvptx::Driver {
        let driver = nvptx::Driver::new().expect("Fail to create compiler-driver");
        nvptx::manifest::generate(driver.path(), &self.crates)
            .expect("Fail to generate Cargo.toml");
        driver
    }
}

const PENE: &[char] = &['(', ')'];
const QUOTE: &[char] = &[' ', '"'];

/// Parse attributes of kernel
///
/// - `crate`: add dependent crate
///    - `#[crate("accel-core")]` equals to `accel-core = "*"` in Cargo.toml
///    - `#[crate("accel-core" = "0.1.0")]` equals to `accel-core = "0.1.0"`
/// - `crate_path`: add dependent crate from local
///    - `#[crate_path("accel-core" = "/some/path")]`
///      equals to `accel-core = { path = "/some/path" }`
pub fn parse_crate(attr: &syn::Attribute) -> Crate {
    let path = &attr.path;
    let path = quote! {#path}.to_string();
    let tts = attr.tokens.to_string();
    let tokens: Vec<_> = tts
        .trim_matches(PENE)
        .split('=')
        .map(|s| s.trim_matches(QUOTE).to_string())
        .collect();
    match path.as_str() {
        "crate" => {
            match tokens.len() {
                // #[crate("accel-core")] case
                1 => Crate {
                    name: tokens[0].clone(),
                    version: None,
                    path: None,
                },
                // #[crate("accel-core" = "0.1.0")] case
                2 => Crate {
                    name: tokens[0].clone(),
                    version: Some(tokens[1].clone()),
                    path: None,
                },
                _ => unreachable!("Invalid line: {:?}", attr),
            }
        }
        "crate_path" => {
            match tokens.len() {
                // #[crate_path("accel-core" = "/some/path")] case
                2 => Crate {
                    name: tokens[0].clone(),
                    version: None,
                    path: Some(fs::canonicalize(&tokens[1]).expect("Fail to normalize")),
                },
                _ => unreachable!("Invalid line: {:?}", attr),
            }
        }
        _ => unreachable!("Unsupported attribute: {:?}", path),
    }
}

// Should I use `cargo::core::dependency::Depenency`?
// https://docs.rs/cargo/0.41.0/cargo/core/dependency/struct.Dependency.html
#[derive(Default, Debug, PartialEq)]
struct Depenency {
    version: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    hash: Option<String>,
}

impl Depenency {
    fn valid(&self) -> bool {
        if self.version.is_some() {
            return true;
        }
        // TODO others
        return false;
    }
}

fn parse_dependency(dep_str: &str) -> Fallible<(String, Depenency)> {
    if let toml::Value::Table(table) = dep_str.parse::<toml::Value>()? {
        for (name, value) in table {
            match value {
                // Like `name = "0.1.1"`
                toml::Value::String(version) => {
                    return Ok((
                        name,
                        Depenency {
                            version: Some(version),
                            ..Default::default()
                        },
                    ));
                }
                // Like `name = { version = "0.1.1" }`
                toml::Value::Table(table) => {
                    let mut dep: Depenency = Default::default();
                    for (key, val) in table {
                        let val = match val {
                            toml::Value::String(val) => val,
                            _ => bail!("Must be string: {}", val),
                        };
                        match key.as_ref() {
                            "version" => dep.version = Some(val),
                            "git" => dep.git = Some(val),
                            _ => bail!("Non supported key: {}", key),
                        }
                    }
                    if dep.valid() {
                        return Ok((name, dep));
                    } else {
                        bail!("Cannot be legalize: {}", dep_str)
                    }
                }
                _ => panic!(""),
            }
        }
        bail!("No entry found");
    } else {
        bail!("Input must be TOML table");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_dependency() {
        let (name, dep) = super::parse_dependency(r#"accel-core = "0.1.1""#).unwrap();
        assert!(dep.valid());
    }
}
