#[macro_use]
extern crate clap;
extern crate reqwest;

mod handler;
mod manga;
mod parser;

use crate::parser::Cli;

use clap::App;
use handler::Handler;
use manga::Manga;
use std::fs;
use std::path::Path;
use threadpool::ThreadPool;

fn main() -> Result<(), Box<std::error::Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let Cli { url, cookie } = parser::parse_cli(&matches);

    let host = url.host().unwrap().to_string();

    let h = Handler::new(&host, &cookie);
    let m = Manga::new(&h, &url);

    println!("Collect Download information");
    let download_urls = m.get_download_urls(&h);

    // starting download
    let pool = ThreadPool::new(16);
    let path = format!("tmp{}", m.number);
    if !Path::new(&path).exists() {
        fs::create_dir(&path)?;
    }
    for (target, filename) in download_urls {
        let path = path.clone();
        let cookie = cookie.clone();
        pool.execute(move || {
            // retry download up to 5 times with verification
            let max_retries = 5;
            for attempt in 1..=max_retries {
                match Handler::download(&target, &path, &filename, &cookie) {
                    Ok(_) => break,
                    Err(ref e) => {
                        println!(
                            "[Attempt {}/{}] Download failed for {}: {}",
                            attempt, max_retries, filename, e
                        );
                        if attempt == max_retries {
                            println!(
                                "Failed to download {} after {} attempts",
                                filename, max_retries
                            );
                        }
                    }
                }
            }
        });
    }

    pool.join();

    Ok(())
}
