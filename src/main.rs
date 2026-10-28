use clap::{Parser, Subcommand};
use km003c_lib::firmware::FirmwareImage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fs, path::PathBuf};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const BASE_URL: &str = "https://okhsunrog.github.io/km003c-resources";

#[derive(Parser)]
#[command(about = "Import and validate KM003C firmware resources")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate packages and register immutable content-based paths.
    Import {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Collection ID from resources.json.
        #[arg(long)]
        source: String,
    },
    /// Validate registered packages and generate public catalogs.
    Build {
        /// Fail if checked-in catalogs differ from validated contents.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u32,
    packages: Vec<Package>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Package {
    path: String,
    sha256: String,
    original_filename: String,
    source: String,
}

#[derive(Serialize)]
struct Catalog {
    schema_version: u32,
    model: &'static str,
    releases: Vec<Release>,
}

#[derive(Serialize)]
struct Release {
    id: String,
    model: String,
    version: String,
    build_date: String,
    path: String,
    url: String,
    sha256: String,
    package_size: usize,
    firmware_size: usize,
    crc32: String,
    notes_en: String,
    notes_zh: String,
    original_filename: String,
    source: String,
}

fn main() -> Result {
    match Args::parse().command {
        Command::Import { files, source } => import(files, source),
        Command::Build { check } => build(check),
    }
}

fn registry() -> Result<Registry> {
    if !PathBuf::from("firmware.json").exists() {
        return Ok(Registry {
            schema_version: 1,
            ..Registry::default()
        });
    }
    let result: Registry = serde_json::from_slice(&fs::read("firmware.json")?)?;
    if result.schema_version != 1 {
        return Err("unsupported firmware registry schema".into());
    }
    Ok(result)
}

fn json<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn token(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.contains("..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(format!("invalid firmware identity component: {value:?}").into());
    }
    Ok(value)
}

fn identity(image: &FirmwareImage, sha: &str) -> Result<(String, String)> {
    let m = image.metadata();
    let version = token(&m.version)?;
    let date = token(&m.date)?;
    let id = format!("km003c-{version}-{date}-{}", &sha[..12]);
    Ok((
        format!("firmware/km003c/{version}/{date}-{sha}.mencrypt"),
        id,
    ))
}

fn resources() -> Result<(serde_json::Value, BTreeSet<String>)> {
    let resources: serde_json::Value = serde_json::from_slice(&fs::read("resources.json")?)?;
    if resources["schema_version"] != 1 {
        return Err("unsupported resources schema".into());
    }
    let mut ids = BTreeSet::new();
    for collection in resources["collections"]
        .as_array()
        .ok_or("missing resource collections")?
    {
        let id = collection["id"].as_str().ok_or("missing collection ID")?;
        if !ids.insert(id.to_owned()) {
            return Err("duplicate resource collection ID".into());
        }
        for asset in collection["assets"]
            .as_array()
            .ok_or("missing collection assets")?
        {
            let sha = asset["sha256"].as_str().ok_or("missing asset SHA256")?;
            if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("invalid asset SHA256".into());
            }
            if !asset["url"]
                .as_str()
                .ok_or("missing asset URL")?
                .starts_with("https://github.com/okhsunrog/km003c-resources/releases/download/")
            {
                return Err("unexpected resource asset URL".into());
            }
        }
    }
    Ok((resources, ids))
}

fn import(files: Vec<PathBuf>, source: String) -> Result {
    let (_, sources) = resources()?;
    if !sources.contains(&source) {
        return Err(format!("unknown source collection: {source}").into());
    }
    let mut registry = registry()?;
    for input in files {
        let bytes = fs::read(&input)?;
        let image = FirmwareImage::parse(&bytes)?;
        image.validate_for_update()?;
        let sha = digest(&bytes);
        let (path, _) = identity(&image, &sha)?;
        let original_filename = input
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("invalid input filename")?
            .to_owned();
        if registry.packages.iter().any(|p| p.sha256 == sha) {
            println!("Already registered: {}", input.display());
            continue;
        }
        let output = PathBuf::from("site").join(&path);
        fs::create_dir_all(output.parent().ok_or("missing output parent")?)?;
        if output.exists() {
            if fs::read(&output)? != bytes {
                return Err("refusing to replace an existing package".into());
            }
        } else {
            use std::io::Write;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)?
                .write_all(&bytes)?;
        }
        registry.packages.push(Package {
            path,
            sha256: sha,
            original_filename,
            source: source.clone(),
        });
        println!(
            "Imported {} {} ({})",
            image.metadata().model,
            image.metadata().version,
            image.metadata().date
        );
    }
    registry.packages.sort_by(|a, b| a.path.cmp(&b.path));
    fs::write("firmware.json", json(&registry)?)?;
    build(false)
}

fn build(check: bool) -> Result {
    let registry = registry()?;
    let (resources, sources) = resources()?;
    let mut releases = Vec::new();
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for package in registry.packages {
        if !package.path.starts_with("firmware/km003c/")
            || package.path.split('/').any(|p| token(p).is_err())
        {
            return Err(format!("invalid package path: {}", package.path).into());
        }
        if !paths.insert(package.path.clone()) {
            return Err("duplicate package path".into());
        }
        if !sources.contains(&package.source) {
            return Err("unknown package source".into());
        }
        let bytes = fs::read(PathBuf::from("site").join(&package.path))?;
        let sha = digest(&bytes);
        if sha != package.sha256 {
            return Err(format!("SHA256 mismatch: {}", package.path).into());
        }
        let image = FirmwareImage::parse(&bytes)?;
        image.validate_for_update()?;
        let (expected_path, id) = identity(&image, &sha)?;
        if package.path != expected_path {
            return Err("package path does not match embedded metadata and hash".into());
        }
        if !ids.insert(id.clone()) {
            return Err("duplicate release identity".into());
        }
        let m = image.metadata();
        releases.push(Release {
            id,
            model: m.model.clone(),
            version: m.version.clone(),
            build_date: m.date.clone(),
            url: format!("{BASE_URL}/{}", package.path),
            path: package.path,
            sha256: sha,
            package_size: bytes.len(),
            firmware_size: m.size,
            crc32: format!("{:08X}", m.crc32),
            notes_en: m.notes_en.clone(),
            notes_zh: m.notes_zh.clone(),
            original_filename: package.original_filename,
            source: package.source,
        });
    }
    let parts = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    releases.sort_by(|a, b| {
        parts(&b.version)
            .cmp(&parts(&a.version))
            .then_with(|| parts(&b.build_date).cmp(&parts(&a.build_date)))
            .then_with(|| a.id.cmp(&b.id))
    });
    let count = releases.len();
    if count == 0 {
        return Err("firmware catalog is empty".into());
    }
    let catalog = Catalog {
        schema_version: 1,
        model: "KM003C",
        releases,
    };
    for (path, expected) in [
        ("site/catalog.json", json(&catalog)?),
        ("site/resources.json", json(&resources)?),
    ] {
        if check {
            if fs::read(path)? != expected {
                return Err(format!("{path} is stale; run resource-index build").into());
            }
        } else {
            fs::write(path, expected)?;
        }
    }
    println!("Validated {count} firmware packages and resource collections.");
    Ok(())
}
