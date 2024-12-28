use regex::Regex;
use reqwest::IntoUrl;
use scraper::{Html, Selector};

const VERSIONS_URL: &str = "https://hub.spigotmc.org/versions";
const VERSION_REGEX: &str = r"^1\.\d{1,2}(?:\.\d{1,2})?$";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0";

async fn get_url<U: IntoUrl>(url: U) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build()?;

    client
        .get(url)
        .send()
        .await
        .expect("Failed to receive response")
        .text()
        .await
}

async fn fetch_url<U: IntoUrl>(url: U) -> Result<Html, reqwest::Error> {
    Ok(Html::parse_document(&(get_url(url).await?)))
}

fn filter_versions(document: Html) -> Vec<String> {
    let version_regex = Regex::new(VERSION_REGEX).unwrap();
    let a_selector = Selector::parse("a").unwrap();

    let mut list: Vec<String> = Vec::new();
    for element in document.select(&a_selector) {
        if let Some(ref_href) = element.value().attr("href") {
            let href = ref_href.strip_suffix(".json").unwrap_or(ref_href);

            if version_regex.is_match(href) {
                list.push(href.to_string());
            }
        }
    }

    list
}

pub async fn fetch_versions() -> Vec<String> {
    filter_versions(fetch_url(VERSIONS_URL).await.unwrap_or_else(|err| {
        panic!("failed fetching versions: {err:#?}")
    }))
}

pub fn is_valid(input: &str) -> bool {
    let regex = Regex::new(VERSION_REGEX).unwrap();
    regex.is_match(input)
}

