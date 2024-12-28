use std::{
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use dirs::data_dir;
use futures_util::StreamExt;
use reqwest::IntoUrl;
use util::StringExt;

const VF_DOWNLOAD_URL: &str =
    "https://github.com/Vineflower/vineflower/releases/download/1.10.1/vineflower-1.10.1.jar";
const VF_FLAGS: [&str; 2] = ["--folder", "--kt-decompile-kotlin=false"];

const ALLOWED_DECOMP_SRC_EXTENSIONS: [&str; 4] = ["sql", "java", "html", "proto"];

mod util;
pub mod version;

#[derive(Debug, Clone, PartialEq)]
pub enum Mapping {
    Mojang,
    Spigot,
}

impl From<String> for Mapping {
    fn from(value: String) -> Self {
        match &*value {
            "Mojang" => Self::Mojang,
            "Spigot/Obfuscated" => Self::Spigot,
            _ => unreachable!(),
        }
    }
}

impl Display for Mapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Mojang => "Mojang",
            Self::Spigot => "Spigot/Obfuscated",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Internals {
    pub mapping: Mapping,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginData {
    pub name: String,
    pub java_version: u8,
    pub jarfile: PathBuf,
    pub output_dir: PathBuf,
    pub version: String,
    pub internals: Option<Internals>,
}

pub async fn download_url<U: IntoUrl, P: AsRef<Path>>(
    url: U,
    path: P,
) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::default();

    let mut stream = client
        .get(url)
        .send()
        .await
        .expect("Failed to receive response")
        .bytes_stream();

    let mut file = File::create(path).expect("Unable to create file");

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("Failed to read bytes");
        file.write_all(&chunk).expect("Failed to write to file");
    }

    Ok(())
}

pub async fn download_vf() -> io::Result<PathBuf> {
    let jarfile = data_dir()
        .expect("data dir could not be found")
        .join("plugdecomp/vineflower.jar");
    if jarfile.exists() {
        return Ok(jarfile);
    }

    if let Some(parent) = jarfile.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Err(err) = download_url(VF_DOWNLOAD_URL, &jarfile).await {
        return Err(io::Error::new(ErrorKind::Other, err.to_string()));
    }

    Ok(jarfile)
}

pub async fn run_vf<P: AsRef<Path>>(jarfile: P, output_dir: P, vf_jarfile: P) -> io::Result<()> {
    let mut command = Command::new("java");
    command.args([
        "-jar",
        vf_jarfile
            .as_ref()
            .to_str()
            .expect("invalid VF jarfile path"),
    ]);
    command.args(VF_FLAGS);
    command.arg(jarfile.as_ref().to_str().expect("invalid jarfile path"));
    command.arg(output_dir.as_ref().to_str().expect("invalid output path"));
    command.stdout(Stdio::null());
    command.stderr(io::stderr());

    let mut process = command.spawn()?;
    let status = process.wait()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::Other,
            format!("VF exited with exit code {status}"),
        ))
    }
}

pub fn filter_resources(java_sourceset: PathBuf, resources_sourceset: PathBuf) -> io::Result<()> {
    fn process_directory(
        current_dir: &PathBuf,
        java_root: &PathBuf,
        resources_root: &PathBuf,
    ) -> io::Result<()> {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let mut path = entry.path();

            if path.is_dir() {
                process_directory(&path, java_root, resources_root)?;
            } else if let Some(extension) = path.extension() {
                let mut extension = extension.to_str().unwrap();

                if extension == "java~" {
                    let old_path = path.clone();
                    path.set_extension("java");
                    extension = "java";
                    fs::rename(old_path, &path)?;
                }

                if !ALLOWED_DECOMP_SRC_EXTENSIONS.contains(&extension) {
                    let relative_path = path.strip_prefix(java_root).unwrap();
                    let destination = resources_root.join(relative_path);

                    if let Some(parent) = destination.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::rename(&path, destination)?;
                }
            }
        }
        Ok(())
    }

    process_directory(&java_sourceset, &java_sourceset, &resources_sourceset)
}

pub fn create_buildscript(data: PluginData) -> String {
    let mut buildscript = String::new();

    buildscript.push_str("plugins {");
    buildscript.push_newline();
    buildscript.push_str("    java");
    buildscript.push_newline();

    if let Some(internals) = &data.internals {
        if internals.mapping == Mapping::Mojang {
            buildscript
                .push_str(r#"    id("io.papermc.paperweight.userdev") version "2.0.0-beta.8""#);

            buildscript.push_newline();
        }
    }
    buildscript.push('}');
    buildscript.push_newline();

    buildscript.push_str("repositories {");
    buildscript.push_newline();
    buildscript.push_str(r#"    maven("https://repo.papermc.io/repository/maven-public")"#);
    buildscript.push_newline();
    buildscript.push('}');
    buildscript.push_newline();

    buildscript.push_str("dependencies {");
    buildscript.push_newline();
    buildscript.push_string(match data.internals {
        Some(internals) => match internals.mapping {
            Mapping::Mojang => format!(
                r#"    paperweight.paperDevBundle("{}-R0.1-SNAPSHOT")"#,
                data.version
            ),
            Mapping::Spigot => format!(
                r#"    compileOnly("org.spigotmc:spigot:{}-R0.1-SNAPSHOT")"#,
                data.version
            ),
        },
        None => {
            format!(
                r#"    compileOnly("io.papermc.paper:paper-api:{}-R0.1-SNAPSHOT")"#,
                data.version
            )
        }
    });
    buildscript.push_newline();
    buildscript.push('}');
    buildscript.push_newline();

    buildscript.push_str("java {");
    buildscript.push_newline();
    buildscript.push_string(format!(
        "    toolchain.languageVersion.set(JavaLanguageVersion.of({}))",
        data.java_version
    ));
    buildscript.push_newline();
    buildscript.push('}');

    buildscript
}

pub fn create_buildsettings(data: PluginData) -> String {
    let mut buildsettings = String::new();

    buildsettings.push_str("rootProject.name = ");
    buildsettings.push_string(format!(r#""{}""#, data.name));
    buildsettings.push_newline();

    buildsettings.push_str("plugins {");
    buildsettings.push_newline();
    buildsettings.push_str(r#"    id("org.gradle.toolchains.foojay-resolver") version "0.9.0""#);
    buildsettings.push_newline();
    buildsettings.push('}');

    buildsettings
}

pub async fn run(data: PluginData) -> Result<(), Box<dyn Error>> {
    let vf_jarfile = download_vf().await?;
    let java_sourceset = data.output_dir.join("src/main/java");
    let resources_sourceset = data.output_dir.join("src/main/resources");

    fs::create_dir_all(&java_sourceset)?;
    fs::create_dir_all(&resources_sourceset)?;

    println!("Decompiling...");
    run_vf(&data.jarfile, &java_sourceset, &vf_jarfile).await?;

    println!("Filtering resources...");
    filter_resources(java_sourceset, resources_sourceset)?;

    println!("Generating build scripts...");
    let buildscript = create_buildscript(data.clone());
    let buildsettings = create_buildsettings(data.clone());

    fs::write(data.output_dir.join("build.gradle.kts"), buildscript)?;
    fs::write(data.output_dir.join("settings.gradle.kts"), buildsettings)?;

    Ok(())
}
