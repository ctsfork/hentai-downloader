use reqwest::header::*;
use std::path::Path;
use std::fmt;

//kimi新增
use reqwest::Proxy;
use reqwest::blocking::{Client};
use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use reqwest::Url;

use clap::App;
use crate::parser;
use crate::parser::Cli;
use crate::parser::ProxyMode;





#[derive(Debug)]
pub enum DownloadError {
    Request(reqwest::Error),
    Io(std::io::Error),
    Verification(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Request(e) => write!(f, "Request error: {}", e),
            DownloadError::Io(e) => write!(f, "IO error: {}", e),
            DownloadError::Verification(msg) => write!(f, "Verification failed: {}", msg),
        }
    }
}

impl DownloadError {
    pub fn is_timeout(&self) -> bool {
        match self {
            DownloadError::Request(e) => e.is_timeout(),
            _ => false,
        }
    }
}


//kimi 新增
impl DownloadError {
    pub fn status(&self) -> Option<reqwest::StatusCode> {
        match self {
            DownloadError::Request(e) => e.status(),
            _ => None,
        }
    }

    // 用来检测错误状态时，是否允许重试，特别是类似404时不允许重试。
    // 如果允许，或者放宽测试条件，如只要网络请求不正确(不管是404,302，或者没有网络)都允许重试，那么可以适当修改该方法中网络错误中的条件。
    pub fn is_retryable(&self) -> bool {
        match self {
            // =========================
            // HTTP / 网络层错误
            // =========================
            DownloadError::Request(e) => {
                // 超时 / 建立连接失败
                if e.is_timeout() || e.is_connect() {
                    return true;
                }

                // // 旧方法： 通过 get_ref() 判断底层 io error - 即实现is_connect()的效果
                // if let Some(io_err) = e.get_ref().and_then(|e| e.downcast_ref::<std::io::Error>()) {
                //     match io_err.kind() {
                //         std::io::ErrorKind::ConnectionRefused
                //         | std::io::ErrorKind::ConnectionReset
                //         | std::io::ErrorKind::ConnectionAborted
                //         | std::io::ErrorKind::NotConnected
                //         | std::io::ErrorKind::TimedOut
                //         | std::io::ErrorKind::BrokenPipe => return true,
                //         _ => {}
                //     }
                // }

                // HTTP 状态码
                if let Some(status) = e.status() {
                    // 429 特殊处理
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        return true;
                    }

                    // 5xx 服务器错误
                    if status.is_server_error() {
                        return true;
                    }

                    // 4xx 不重试
                    return false;
                }

                false
            }


            // =========================
            // 下载过程中 IO 错误
            // =========================
            DownloadError::Io(e) => {
                // 🔴 磁盘满（必须优先判断）
                if let Some(code) = e.raw_os_error() {
                    // Linux/macOS = 28
                    // Windows = 112
                    if code == 28 || code == 112 {
                        return false;
                    }
                }

                match e.kind() {
                    // 可恢复 IO 错误
                    std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof => true,

                    _ => false,
                }
            }

           
            // =========================
            // 文件校验失败
            // =========================
            DownloadError::Verification(_) => false,
        }
    }
}




#[derive(Clone)]
pub struct Handler {
    // pub client: reqwest::Client,
    pub client: Client,
    pub host: String,
    cookie: String,
}


impl Handler {

    // 根据参数选择对应的代理配置
    fn build_client() -> Client {
        //加载解析参数配置文件
        let yaml = load_yaml!("cli.yml");
        let matches = App::from_yaml(yaml).get_matches();
        let cli: Cli = parser::parse_cli(&matches);
        // println!("CLI -> {:?}", cli);

        // 1️⃣ 最高优先级：--proxy
        if let Some(proxy_url) = &cli.proxy {
             println!("Using custom proxy: {}", proxy_url);
             // println!("准备配置自定义代理服务: {}", proxy_url);
            return Self::apply_custom_proxy(&proxy_url,&cli);
        } else {
            // 2️⃣ 根据 proxy-mode
            match cli.proxy_mode {
                ProxyMode::None => { 
                    println!("Proxy mode: none (no proxy)");
                    return Client::new();
                }
                ProxyMode::Http => { 
                    println!("Proxy mode: http (env)");
                    // println!("准备配置HTTP代理服务，从环境变量中获取代理地址......");
                    return Self::apply_http_env_proxy();
                }
                ProxyMode::Socks => { 
                    println!("Proxy mode: socks (env)");
                    // println!("准备配置SOCKS代理服务，从环境变量中获取代理地址......");
                    return Self::apply_socks_env_proxy(&cli);
                }
            }
        }

    }


    // 读取环境变量(http_proxy|https_proxy)的值配置Proxy::http，Proxy::https代理服务。
    fn apply_http_env_proxy() -> Client{
        let mut builder = Client::builder();

        if let Ok(http_proxy) = std::env::var("http_proxy")
            .or_else(|_| std::env::var("HTTP_PROXY"))
        {
            println!("HTTP proxy found: {}", http_proxy);
             // println!("从环境中找到HTTP代理地址: {}", http_proxy);
            if let Ok(proxy) = Proxy::http(&http_proxy) {
                builder = builder.proxy(proxy);
                // println!("HTTP代理，配置成功！");
                println!("HTTP proxy, configuration successful!");
            }else{
                // println!("HTTP代理，配置失败！");
                println!("HTTP proxy, configuration failed!");
            }
        } else {
            // println!("No HTTP proxy found in environment");
            println!("No HTTP_PROXY|http_proxy found in environment");
            println!("HTTP proxy, configuration failed!");

            // println!("没有在环境中找到HTTP代理地址，环境变量名应该为：http_proxy 或 HTTP_PROXY");
            // println!("HTTP代理，配置失败！");
        }

        if let Ok(https_proxy) = std::env::var("https_proxy")
            .or_else(|_| std::env::var("HTTPS_PROXY"))
        {
            println!("HTTPS proxy found: {}", https_proxy);
            // println!("从环境中找到HTTPS代理地址: {}", https_proxy);
            if let Ok(proxy) = Proxy::https(&https_proxy) {
                builder = builder.proxy(proxy);
                // println!("HTTPS代理，配置成功！");
                println!("HTTPS proxy, configuration successful!");
            }else{
                // println!("HTTPS代理，配置失败！");
                println!("HTTPS proxy, configuration failed!");
            }
        } else {
            // println!("No HTTPS proxy found in environment");
            println!("No HTTPS_PROXY|https_proxy found in environment");
            println!("HTTPS proxy, configuration failed!");

             // println!("没有在环境中找到HTTP代理地址，环境变量名应该为：https_proxy 或 HTTPS_PROXY");
             // println!("HTTPS代理，配置失败！");
        }

        builder.build().unwrap()
    }

    // 读取环境变量(all_proxy)的值配置Proxy::all代理服务
    fn apply_socks_env_proxy(cli:&Cli) -> Client{
        let mut builder = Client::builder();

        if let Ok(mut proxy_url) = std::env::var("all_proxy")
            .or_else(|_| std::env::var("ALL_PROXY"))
        {
            println!("SOCKS proxy found: {}", proxy_url);
            // println!("从环境中找到SOCKS代理地址: {}", proxy_url);

            if cli.convert_socks5h {
                if proxy_url.starts_with("socks5://") {
                    proxy_url = proxy_url.replacen("socks5://", "socks5h://", 1);
                    println!("Convert socks5 to socks5h: {}", proxy_url);
                    // println!("已将socks5地址转换成socks5h地址：{}", proxy_url);
                }
            }

            if let Ok(proxy) = Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
                // println!("SOCKS代理，配置成功!");
                println!("SOCKS proxy, configuration successful!");
            }else{
                // println!("SOCKS代理，配置失败!");
                println!("SOCKS proxy, configuration failed!");
            }
        } else {
            println!("No ALL_PROXY|all_proxy found in environment");
            println!("SOCKS proxy, configuration failed!");

            // println!("没有在环境中找到SOCKS代理地址，环境变量名应该为：all_proxy 或 ALL_PROXY");
            // println!("SOCKS代理，配置失败!");
        }

        builder.build().unwrap()
    }

    // 根据自定义地址配置对应的代理服务器
    fn apply_custom_proxy(proxy_url: &str, cli:&Cli) -> Client{
        let mut builder = Client::builder();

        if proxy_url.starts_with("http://") || proxy_url.starts_with("https://") {
            // println!("Custom HTTP proxy");
            println!("Configure HTTP/HTTPS proxy: {}", url);
            // println!("准备配置HTTP/HTTPS代理......");

            if let Ok(proxy_http) = Proxy::http(proxy_url) {
                builder = builder.proxy(proxy_http);
                // println!("HTTP代理，配置成功！");
                println!("HTTP proxy, configuration successful!");
            }else{
                // println!("HTTP代理，配置失败！");
                println!("HTTP proxy, configuration failed!");
            }

            if let Ok(proxy_https) = Proxy::https(proxy_url) {
                builder = builder.proxy(proxy_https);
                // println!("HTTPS代理，配置成功！");
                println!("HTTPS proxy, configuration successful!");
            }else{
                // println!("HTTPS代理，配置失败！");
                println!("HTTPS proxy, configuration failed!");

            }

        } else if proxy_url.starts_with("socks5://") || proxy_url.starts_with("socks5h://") {
            let mut url = proxy_url.to_string();
            // println!("Custom SOCKS proxy: {}", url);
            println!("Configure SOCKS proxy: {}", url);
            // println!("准备配置SOCKS代理......");

            if cli.convert_socks5h {
                if url.starts_with("socks5://") {
                    url = url.replacen("socks5://", "socks5h://", 1);
                    println!("Convert socks5 to socks5h: {}", url);
                    // println!("已将socks5地址转换成socks5h地址：{}", url);
                }
            }


            if let Ok(proxy) = Proxy::all(&url) {
                builder = builder.proxy(proxy);
                println!("SOCKS proxy, configuration successful!");
                // println!("SOCKS代理，配置成功!");
            }else{
                println!("SOCKS proxy, configuration failed!");
                // println!("SOCKS代理，配置失败!");
            }
        } else {
            println!("Unsupported proxy scheme: {}", proxy_url);
            // println!("自定义代理配置失败，不支持的代理方案: {}", proxy_url);
        }

        builder.build().unwrap()
    }

}



// //kimi old
// impl Handler {

//     //kimi新增-检测环境变量中是否存在all_proxy相关配置
//     fn has_env_proxy() -> bool {
//     std::env::var("HTTPS_PROXY").is_ok()
//         || std::env::var("https_proxy").is_ok()
//         || std::env::var("HTTP_PROXY").is_ok()
//         || std::env::var("http_proxy").is_ok()
//         || std::env::var("ALL_PROXY").is_ok()
//         || std::env::var("all_proxy").is_ok()
//     }

//     fn build_client() -> Client {
//         // 环境变量中没有代理配置信息
//         if !Self::has_env_proxy() {
//             // 没有代理 → 直接默认 client
//             return Client::new();
//         }

//         let mut client = Client::builder();
        
//         if let Ok(proxy_url) = std::env::var("http_proxy")
//             .or_else(|_| std::env::var("HTTP_PROXY"))
//         {
//             println!("准备配置HTTP_PROXY代理  ->  url: {}",proxy_url);
//             if let Ok(proxy) = Proxy::http(&proxy_url) {
//                 client = client.proxy(proxy);
//                  eprintln!("配置了HTTP_PROXY代理......");
//             }
//         }
//         if let Ok(proxy_url) = std::env::var("https_proxy")
//             .or_else(|_| std::env::var("HTTPS_PROXY"))
//         {
//             println!("准备配置HTTPS_PROXY代理  ->  url: {}",proxy_url);
//             if let Ok(proxy) = Proxy::https(&proxy_url) {
//                 client = client.proxy(proxy);
//                  eprintln!("配置了HTTPS_PROXY代理......");
//             }
//         }
//         if let Ok(proxy_url) = std::env::var("ALL_PROXY")
//             .or_else(|_| std::env::var("all_proxy"))
//         {
//             // let proxy_url = proxy_url.replace("socks5://", "socks5h://");
//              println!("准备配置ALL_PROXY代理  ->  url: {}",proxy_url);
//             if let Ok(proxy) = Proxy::all(&proxy_url) {
//                 client = client.proxy(proxy);
//                 eprintln!("配置了ALL_PROXY代理......");
//             }
//         }


//         client.build().unwrap()
//     }

// }




impl Handler {

    pub fn new(host: &str, cookie: &str) -> Self {
        Handler {
            //修改前
            // client: reqwest::Client::new(),
            //Kimi修改后
            client: Self::build_client(),
            host: host.to_string(),
            cookie: cookie.to_string(),
        }
    }


    //kimi新增 
    fn build_cookie(&self) -> String {
        let mut map = HashMap::new();

        // 1️⃣ 默认 nw=, 表示允许下载受限制或被标记为具有攻击性的图集
        let defaults = [
            ("nw", "1"),
            // ("theme", "dark"),
        ];
        for (k, v) in defaults {
            map.insert(k.to_string(), v.to_string());
        }


        // 2️⃣ 解析用户 cookie
        for part in self.cookie.split(';') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some((key, value)) = trimmed.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        // 3️⃣ 重建 cookie 字符串
        map.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }


    pub fn request(&self, _task: &str, url: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
        // let res = self
        //     .client
        //     .get(url)
        //     .header(COOKIE, &self.cookie[..])
        //     .header(HOST, &self.host[..])
        //     .header(
        //         USER_AGENT,
        //         "Mozilla/5.0 (X11; Linux x86_64; rv:65.0) Gecko/20100101 Firefox/65.0",
        //     )
        //     .send();
        // res


        //kimi修改 - 与 is_retryable 相关联
        let res = self
            .client
            .get(url)
            .header(COOKIE, self.build_cookie())
            .header(HOST, &self.host[..])
            .header(
                USER_AGENT,
                "Mozilla/5.0 (X11; Linux x86_64; rv:65.0) Gecko/20100101 Firefox/65.0",
            )
            .send()?                // 网络错误
            .error_for_status();    // 让 HTTP 非 2xx 成为错误，如果没该方法，那么形如404也会返回为成功
        res
    }

    pub fn download(target: &str, path: &str, filename: &str, cookie: &str) -> Result<(), DownloadError> {
        //Kimi 新增 - 检查文件是否存在-如果存在则跳过本次下载
        let fname = Path::new(path).join(filename);

        // 🔴 第一步：检查是否已存在
        if fname.exists() {
            println!("File already exists: {}, skipping download.", fname.display());
            return Ok(());
        }


        // Extract host from target URL for proper headers
        let host = reqwest::Url::parse(target)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();

        let dh = Handler::new(&host, cookie);
        let mut res = match dh.request("Download", &target) {
            Ok(r) => r,
            Err(e) => return Err(DownloadError::Request(e)),
        };

        // 修改前
        // let fname = Path::new(path).join(filename);
        let mut dest = match std::fs::File::create(&fname) {
            Ok(f) => f,
            Err(e) => return Err(DownloadError::Io(e)),
        };

        println!("Downloading: {}", fname.to_str().unwrap());
        if let Err(e) = std::io::copy(&mut res, &mut dest) {
            //当 copy 失败时，必须删除半文件
            let _ = std::fs::remove_file(&fname);
            return Err(DownloadError::Io(e));
        }

        // Verify download: check file exists and has content
        Self::verify_download(&fname)?;

        Ok(())
    }

    fn verify_download(path: &Path) -> Result<(), DownloadError> {
        // Check file exists
        if !path.exists() {
            return Err(DownloadError::Verification(format!(
                "File does not exist: {}",
                path.display()
            )));
        }

        // Check file size (at least 1KB)
        let metadata = std::fs::metadata(path).map_err(DownloadError::Io)?;
        let min_size = 1024; // 1KB minimum
        if metadata.len() < min_size {
            let _ = std::fs::remove_file(path);
            return Err(DownloadError::Verification(format!(
                "File too small ({} bytes, min {} bytes): {}",
                metadata.len(),
                min_size,
                path.display()
            )));
        }

        // Verify image magic bytes
        if !Self::is_valid_image(path)? {
            let _ = std::fs::remove_file(path);
            return Err(DownloadError::Verification(format!(
                "File is not a valid image: {}",
                path.display()
            )));
        }

        println!("Verified: {} ({} bytes)", path.display(), metadata.len());
        Ok(())
    }

    fn is_valid_image(path: &Path) -> Result<bool, DownloadError> {
        use std::io::Read;

        let mut file = std::fs::File::open(path).map_err(DownloadError::Io)?;
        let mut header = [0u8; 12];
        file.read_exact(&mut header).map_err(DownloadError::Io)?;

        // Check magic bytes for common image formats
        // JPEG: FF D8 FF
        // PNG: 89 50 4E 47 0D 0A 1A 0A
        // GIF: 47 49 46 38 (GIF8)
        // WebP: 52 49 46 46 ... 57 45 42 50 (RIFF....WEBP)

        let is_jpeg = header[0] == 0xFF && header[1] == 0xD8 && header[2] == 0xFF;
        let is_png = header[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let is_gif = header[0..4] == [0x47, 0x49, 0x46, 0x38];
        let is_webp = header[0..4] == [0x52, 0x49, 0x46, 0x46]
            && header[8..12] == [0x57, 0x45, 0x42, 0x50];

        Ok(is_jpeg || is_png || is_gif || is_webp)
    }
}
