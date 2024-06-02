use reqwest::blocking::Client;
use serde_json::Value;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::time::Duration;

const BASE_URL: &str = "https://raw.githubusercontent.com/arkadiyt/bounty-targets-data/main/data";

struct Platform {
    url: String,
    is_bbp: fn(&Value) -> bool,
    extract_scope: fn(&Value) -> String,
}

impl Platform {
    fn process(
        &self,
        old_scopes: &Vec<String>,
    ) -> Result<serde_json::Map<String, Value>, Box<dyn std::error::Error>> {
        let json_data = self.fetch_json_data().unwrap();
        let mut data = serde_json::Map::new();

        for program in json_data.as_array().unwrap() {
            let mut name = program
                .get("name")
                .unwrap()
                .as_str()
                .unwrap()
                .trim_matches(&['"'])
                .to_string();
            
            let _type = if (self.is_bbp)(program) { "💸" } else { "" };
            name = format!("{name} {_type}");
            let url = program.get("url").unwrap();
            let in_scopes = program.get("targets").unwrap().get("in_scope").unwrap();

            let mut scopes: Vec<String> = vec![];
            let mut is_name_printed = false;

            for scope in in_scopes.as_array().unwrap() {
                let s = (self.extract_scope)(scope);

                if !old_scopes.contains(&s) {
                    if !is_name_printed {
                        println!(
                            "# {name}\n[Policy]({})",
                            url.to_string().trim_matches(&['"'])
                        );
                        is_name_printed = true;
                    }

                    // print new assets
                    println!("{s}");
                }

                scopes.push(s);
            }

            data.insert(name, serde_json::to_value(scopes)?);
        }
        Ok(data)
    }

    fn fetch_json_data(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Create a reqwest Client 30s timeout
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        // Send a GET request
        let mut response = client.get(&self.url).send()?;

        if response.status().is_success() {
            // Read the response body
            let mut body = String::new();
            let _ = response.read_to_string(&mut body);

            let data: serde_json::Value = serde_json::from_str(&body)?;
            Ok(data)
        } else {
            eprintln!("Failed to fetch URL: {}", response.status());
            std::process::exit(1);
        }
    }
}

fn load_data(file_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // Open the file
    let mut file = match File::open(file_name) {
        Ok(file) => file,
        Err(_) => File::create(file_name)?
    };

    // Read the file
    let mut yaml_str = String::new();
    file.read_to_string(&mut yaml_str)?;

    // Parse YAML data
    let yaml_data: serde_yaml::Value = serde_yaml::from_str(&yaml_str)?;

    let mut old_scopes: Vec<String> = vec![];

    for (_, platform_data) in yaml_data.as_mapping().unwrap() {
        for (__, in_scopes) in platform_data.as_mapping().unwrap() {
            for scope in in_scopes.as_sequence().unwrap() {
                old_scopes.push(scope.as_str().unwrap().to_string());
            }
        }
    }
    Ok(old_scopes)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let h1: Platform = Platform {
        url: format!("{}/hackerone_data.json", BASE_URL),
        is_bbp: |program| {
            program
                .get("offers_bounties")
                .and_then(|v: &Value| v.as_bool())
                .unwrap_or(false)
        },
        extract_scope: |scope| {
            scope
                .get("asset_identifier")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        },
    };

    let bc: Platform = Platform {
        url: format!("{}/bugcrowd_data.json", BASE_URL),
        is_bbp: |program| {
            program
                .get("max_payout")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                != 0
        },
        extract_scope: |scope| {
            scope
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        },
    };

    // Process arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <FILE>", args[0]);
        std::process::exit(1);
    }

    // Read old data
    let file_name = &args[1];
    let old_scopes = load_data(file_name)?; // Store old assets in a new array

    let mut json_data = serde_json::Map::new();

    // Process data
    let h1_data = h1.process(&old_scopes)?;
    let bc_data = bc.process(&old_scopes)?;
    json_data.insert("HackerOne".to_string(), serde_json::Value::Object(h1_data));
    json_data.insert("BugCrowd".to_string(), serde_json::Value::Object(bc_data));

    // Save new data
    let yaml_data = serde_yaml::to_string(&json_data)?;
    let mut output_file = File::create(file_name)?;
    output_file.write_all(yaml_data.as_bytes())?;

    Ok(())
}
