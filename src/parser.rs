use std::path::Path;
use std::fs;
// use url::{Host, Url};

//kimi
use reqwest::Url;
use url::Host;



#[derive(Debug)]
pub struct Cli {
    pub url: Url,
    pub cookie: String,
    pub retry: bool,

    pub proxy_mode: ProxyMode,
    pub proxy: Option<String>,
    pub convert_socks5h: bool,
}


#[derive(Debug, Clone)]
pub enum ProxyMode {
    None,
    Http,
    Socks,
}


pub fn parse_cli(matches: &clap::ArgMatches) -> Cli {
    // parse url: String to download_url: url::Url
    let url: String = matches
        .value_of("url")
        .expect("Should provide the url.")
        .parse::<String>()
        .expect("Incorrect url.");

    let download_url: Url = Url::parse(url.trim()).expect("Parse url failed");
    assert!(download_url.scheme() == "https");
    assert!(
        download_url.host() == Some(Host::Domain("e-hentai.org"))
            || download_url.host() == Some(Host::Domain("exhentai.org"))
    );


    // read cookie file into cookie: String
    let mut cookie = String::from("");
    if let Some(c) = matches.value_of("cookie") {
        if Path::new(&c).exists() {
            cookie = fs::read_to_string(&c)
                .expect("Something went wrong reading the cookie file")
                .trim()
                .to_string();
        }
    }



    // -------------------------
    // 3️⃣ retry
    // -------------------------
    let retry = matches.is_present("retry");


    // -------------------------
    // 4️⃣ proxy-mode
    // -------------------------
    let proxy_mode = match matches.value_of("proxy-mode").unwrap_or("none") {
        "none" => ProxyMode::None,
        "http" => ProxyMode::Http,
        "socks" => ProxyMode::Socks,
        _ => unreachable!(),
    };


    // -------------------------
    // 5️⃣ proxy
    // -------------------------
    let proxy = if let Some(proxy_str) = matches.value_of("proxy") {
        let proxy_str = proxy_str.trim();
        let parsed = Url::parse(proxy_str)
            .expect("Invalid proxy URL format");

        match parsed.scheme() {
            "http" | "https" | "socks5" | "socks5h" => {}
            _ => {
                eprintln!(
                    "Error: unsupported proxy scheme '{}'.\n\
                     Supported schemes: http, https, socks5, socks5h",
                    parsed.scheme()
                );
                std::process::exit(1);
            }
        }

        Some(proxy_str.to_string())
    } else {
        None
    };



    let convert_socks5h = matches.is_present("convert-socks5h");



    Cli {
        url: download_url,
        cookie,
        retry,
        proxy_mode,
        proxy,
        convert_socks5h,
    }
}
