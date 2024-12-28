use std::{error::Error, path::PathBuf};

use clap::Parser;
use plugdecomp::{
    run, version::fetch_versions, Internals, Mapping, PluginData
};
use promkit::preset::{
    confirm::Confirm, listbox::Listbox, query_selector::QuerySelector, readline::Readline,
};

const JAVA_VERSIONS: [u8; 6] = [8, 11, 17, 21, 22, 23];

/// plugdecomp - A tool for setting up gradle workspaces
/// for decompiled Minecraft plugins.
#[derive(Parser)]
struct Cli {
    /// The input jar file.
    pub jarfile: PathBuf,
    /// The output directory.
    pub output_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let Cli { jarfile , output_dir} = Cli::parse();

    let mut name_prompt = Readline::default()
        .title("What is the name of the project?")
        .prompt()?;
    let name = name_prompt.run()?;

    let mut version_prompt =
        QuerySelector::new(fetch_versions().await, |query, items| -> Vec<String> {
            if query.is_empty() {
                items.clone()
            } else {
                items
                    .iter()
                    .filter(|version| version.to_lowercase().contains(&query.to_lowercase()))
                    .map(|v| v.to_string())
                    .collect()
            }
        })
        .title("What Minecraft version does it use?")
        .listbox_lines(5)
        .prompt()?;
    let version = version_prompt.run()?;

    let mut java_version_prompt = QuerySelector::new(JAVA_VERSIONS, |text, items| {
        text.parse::<u8>()
            .map(|query| {
                items
                    .iter()
                    .filter(|num| query <= num.parse::<u8>().unwrap_or_default())
                    .map(|num| num.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or(items.clone())
    })
    .title("What Java version does it use?")
    .listbox_lines(5)
    .prompt()?;
    let java_version = java_version_prompt.run()?.parse::<u8>().expect("invalid java version");

    let mut internals = None;
    let mut internals_prompt = Confirm::new("Does this plugin use internals?").prompt()?;
    let mut mapping_prompt = Listbox::new(vec![Mapping::Mojang, Mapping::Spigot])
        .title("What mappings does this plugin use?")
        .listbox_lines(2)
        .prompt()?;

    let using_internals = internals_prompt.run()?;
    if ["yes", "no", "y"].contains(&&*using_internals) {
        let mappings = Mapping::from(mapping_prompt.run()?);

        internals = Some(Internals { mapping: mappings })
    }

    let data = PluginData {
        name,
        java_version,
        jarfile,
        output_dir,
        version,
        internals
    };

    println!();

    run(data).await
}
