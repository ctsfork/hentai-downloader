extern crate reqwest;
use crate::handler::Handler;
use core::cmp::max;
use select::document::Document;
use select::predicate::Name;
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;

pub struct Manga {
    pub number: u32,
    pub pages: u32,
    pub url: reqwest::Url,
}

impl Manga {
    pub fn new(h: &Handler, url: &reqwest::Url) -> Self {
        Manga {
            number: Self::get_hentai_number(url),
            pages: Self::get_page_number(h, url),
            url: url.clone(),
        }
    }
    fn get_hentai_number(url: &reqwest::Url) -> u32 {
        let segs = url.path_segments().map(|c| c.collect::<Vec<_>>()).unwrap();

        segs[1].parse::<u32>().expect("Incorrect url")
    }
    fn get_page_number(h: &Handler, url: &reqwest::Url) -> u32 {
        // let mut pages = 0;
        // let res = h
        //     .request("Get Page number", &url[..])
        //     .expect("Get page number failed");
        // Document::from_read(res)
        //     .expect("Document read response failed.")
        //     .find(Name("a"))
        //     .filter_map(|n| n.attr("href"))
        //     .for_each(|x| {
        //         if x.contains("?p=") {
        //             let num = x.split("?p=").last();
        //             pages = max(num.unwrap_or("0").parse::<u32>().unwrap_or(0), pages);
        //         }
        //     });
        // pages



        let res = h
            .request("Get Page number", &url[..])
            .expect("Get page number failed");

        let status = res.status();
        let body = res.text().expect("Failed to read body");

        // 🔴 检查是否被封
        if status.as_u16() == 503
            || body.contains("This IP address has been temporarily banned due to an excessive request rate")
        {
            eprintln!("Your IP has been temporarily banned.");
            eprintln!("Consider:");
            eprintln!("  • Wait for a while and try again");
            eprintln!("  • Switch IP and try again");
            std::process::exit(1);
        }


        let mut pages = 0;

        Document::from(body.as_str())
            .find(Name("a"))
            .filter_map(|n| n.attr("href"))
            .for_each(|x| {
                if let Some(pos) = x.find("?p=") {
                    let num_str = &x[pos + 3..];
                    if let Ok(num) = num_str.parse::<u32>() {
                        pages = max(num, pages);
                    }
                }
            });
        pages
    }

    /// Returns (image_url, filename)
    /// filename is extracted from page URL (e.g., "3729116-3" from ".../s/xxx/3729116-3")
    /// with extension from the actual image URL
    fn get_image_link(url: &str, h: Handler) -> (String, String) {
        let tmp = match &(h.host.to_string())[..] {
            "exhentai.org" => "exhentai",
            "e-hentai.org" => "ehgt",
            _ => panic!("shound not happend"),
        };
        let res = h
            .request("Get image link", url)
            .expect("Get image link failed");

        let mut ret: Option<String> = None;
        Document::from_read(res)
            .unwrap()
            .find(Name("img"))
            .filter_map(|n| n.attr("src"))
            .for_each(|x| {
                if !x.contains(tmp) {
                    ret = Some(x.to_string());
                    println!("Find {}", x);
                }
            });

        let image_url = match ret {
            Some(x) => x,
            None => panic!("Get image Failed with url: {}", url),
        };

        // Extract filename from page URL (last segment, e.g., "3729116-3")
        let base_name = url
            .trim_end_matches('/')
            .split('/')
            .last()
            .unwrap_or("unknown");

        // Extract extension from image URL
        let extension = image_url
            .split('/')
            .last()
            .and_then(|s| s.split('.').last())
            .unwrap_or("jpg");

        let filename = format!("{}.{}", base_name, extension);
        (image_url, filename)
    }

    /// Returns Vec<(image_url, filename)>
    pub fn get_download_urls<'a>(&self, h: &'a Handler) -> Vec<(String, String)> {
        let url = &self.url;
        let pages = self.pages;

        let download_urls = Arc::new(Mutex::new(vec![]));

        for i in 0..pages + 1 {
            let pool = ThreadPool::new(8);

            let download_url = url.join(&format!("?p={}", i)).unwrap();
            let res = h
                .request("Get each page", &download_url.as_str())
                .expect("Get each page failed");

            let mut links: Vec<String> = vec![];
            Document::from_read(res)
                .expect("Document read response failed.")
                .find(Name("a"))
                .filter_map(|n| n.attr("href"))
                .for_each(|x| {
                    if x.contains("s/") {
                        links.push(x.to_string());
                    }
                });

            for link in links {
                let cloned_v = download_urls.clone();
                let g = h.clone();
                pool.execute(move || {
                    let (image_url, filename) = Manga::get_image_link(&link, g);
                    cloned_v.lock().unwrap().push((image_url, filename));
                });
            }

            pool.join();
        }

        let lock = Arc::try_unwrap(download_urls).expect("Lock still has multiple owners");
        let urls = lock.into_inner().expect("Mutex cannot be locked");
        urls
    }
}
