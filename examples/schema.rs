#[path = "../src/config.rs"]
mod config;

fn main() {
    let settings = schemars::generate::SchemaSettings::draft07();
    let generator = schemars::generate::SchemaGenerator::new(settings);
    let schema = generator.into_root_schema_for::<config::Config>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
