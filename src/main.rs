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


//kimi 新增
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};




//kimi - 实现指数退避（Exponential Backoff）
fn jitter() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();

    (nanos % 300) as u64
}

fn compute_backoff(attempt: u32) -> Duration {
    let base: u64 = 500;       // 500ms 起始
    let max_delay: u64 = 10_000; // 最大 10s

    // 2^attempt，但最多放大到 2^5
    let exp = base * 2_u64.pow(attempt.min(5));

    let delay = (exp + jitter()).min(max_delay);

    Duration::from_millis(delay)
}




fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let Cli { url, cookie } = parser::parse_cli(&matches);

    let host = url.host().unwrap().to_string();

    let h = Handler::new(&host, &cookie);
    let m = Manga::new(&h, &url);

    println!("Collect Download information");
    let download_urls = m.get_download_urls(&h);

    //未获取到页面数据-可能是缺少cookie或者这是一个存在着”内容警告“的图集，就会下载失败
    //例如: https://e-hentai.org/g/3809093/c06ff2b95a/
    if download_urls.is_empty() {
        eprintln!("Error: No downloadable resources found.");
        eprintln!("Possible reasons:");
        // 1️⃣ 图集不存在
        eprintln!("  • The gallery may not exist.");
        // 2️⃣ 被标记为受限制
        eprintln!("  • The gallery may be flagged as restricted or containing offensive content and cannot be downloaded.");
        // 3️⃣ 特殊域名提示
        if host == "exhentai.org" {
            if cookie.trim().is_empty() {
                eprintln!("  • Accessing exhentai.org requires a valid login cookie.");
                eprintln!("    Please provide one using: -c <cookie_file>");
            }
        }
        std::process::exit(1);
    }


    let path = format!("tmp{}", m.number);
    if !Path::new(&path).exists() {
        fs::create_dir(&path)?;
    }


    //-r | --retry 参数获取
    let force_retry = matches.is_present("retry");

    //
    let mut pending_tasks = download_urls;

    //新增循环逻辑
    loop {
        let pool = ThreadPool::new(16);
        let failed_tasks = Arc::new(Mutex::new(Vec::new()));

        // for (target, filename) in pending_tasks.clone() {
        for (target, filename) in pending_tasks.iter().cloned(){
            let path = path.clone();
            let cookie = cookie.clone();
            let failed_tasks = Arc::clone(&failed_tasks);

            pool.execute(move || {
                let max_retries = 5;

                let mut success = false;

                for attempt in 1..=max_retries {
                    match Handler::download(&target, &path, &filename, &cookie) {
                        Ok(_) => {
                            success = true;
                            break;
                        }
                        Err(ref e) => {
                            // 判断是否应该重试 - 如果不需要判断只要失败就重试，只需要注释掉该代码即可。
                            if !e.is_retryable() {
                                println!(
                                    "Non-retryable error for {}: {}",
                                    filename, e
                                );
                                break; // 直接放弃，不进入重试列表
                            }



                            // println!(
                            //     "[Attempt {}/{}] Download failed for {}: {}",
                            //     attempt, max_retries, filename, e
                            // );
                            // if attempt == max_retries {
                            //     println!(
                            //         "Failed to download {} after {} attempts",
                            //         filename, max_retries
                            //     );
                            // }
                            // thread::sleep(Duration::from_secs(1));



                            if attempt == max_retries {
                                println!(
                                    "Failed after {} attempts: {} ({})",
                                    max_retries, filename, e
                                );
                                break;
                            }

                            let delay = compute_backoff(attempt as u32);
                            println!(
                                "[Attempt {}/{}] {} failed: {}. Retrying in {:?}",
                                attempt, max_retries, filename, e, delay
                            );
                            
                            thread::sleep(delay);
                        }
                    }
                }

                if !success {
                    let mut lock = failed_tasks.lock().unwrap();
                    lock.push((target.clone(), filename.clone()));
                }
            });
        }

        pool.join();

        let retry_list = Arc::try_unwrap(failed_tasks)
            // .unwrap()
            .expect("Arc still has multiple owners")
            .into_inner()
            .unwrap();

        if retry_list.is_empty() {
            println!("All downloads completed successfully.");
            break;
        }

        if !force_retry {
            println!("Some downloads failed. Use -r to force retry.");
            break;
        }

        println!(
            "Retrying {} failed downloads after delay...",
            retry_list.len()
        );


        thread::sleep(Duration::from_secs(5));


        pending_tasks = retry_list;
    }


    Ok(())

}
