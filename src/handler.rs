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

    //kimi新增-检测环境变量中是否存在all_proxy相关配置
    fn has_env_proxy() -> bool {
    std::env::var("HTTPS_PROXY").is_ok()
        || std::env::var("https_proxy").is_ok()
        || std::env::var("HTTP_PROXY").is_ok()
        || std::env::var("http_proxy").is_ok()
        || std::env::var("ALL_PROXY").is_ok()
        || std::env::var("all_proxy").is_ok()
    }


    //kimi-检查代理端口是否可用，一般用来判断环境变量中配置的代理服务地址的端口是否启用。
    fn check_proxy_alive(proxy_url: &str) -> bool {
        Url::parse(proxy_url)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?;
            let port = url.port_or_known_default()?;
            (host, port).to_socket_addrs().ok()?.next()
        })
        .map(|addr| {
            TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok()
        })
        .unwrap_or(false)


        // if let Ok(url) = Url::parse(proxy_url) {
        //     if let Some(host) = url.host_str() {
        //         if let Some(port) = url.port_or_known_default() {
        //             if let Ok(mut addrs) = (host, port).to_socket_addrs() {
        //                 if let Some(addr) = addrs.next() {
        //                     return TcpStream::connect_timeout(
        //                         &addr,
        //                         Duration::from_secs(2),
        //                     )
        //                     .is_ok();
        //                 }
        //             }
        //         }
        //     }
        // }
        // false
    }



    //kimi 新增
    fn build_client() -> Client {
        // 环境变量中没有代理配置信息
        if !Self::has_env_proxy() {
            // 没有代理 → 直接默认 client
            return Client::new();
        }



        // 环境变量中配置了代理信息，检查其对应的代理端口是否启用
        let proxy_vars = [
            "ALL_PROXY",
            "all_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "HTTPS_PROXY",
            "https_proxy",
        ];
        let mut is_enable_port = false;
        for var in proxy_vars {
            if let Ok(proxy_url) = std::env::var(var) {
                if Self::check_proxy_alive(&proxy_url) {
                    is_enable_port = true;
                    break;
                } 
            }
        }
        //如果都不可用则不创建Proxy配置
        if !is_enable_port {
            return Client::new();
        }



        let mut client = Client::builder();

        /*
        添加顺序是：
            1️⃣ HTTPS_PROXY
            2️⃣ HTTP_PROXY
            3️⃣ ALL_PROXY
        而在 reqwest 里：
            .proxy() 是添加规则，不是覆盖
            允许多个 proxy 规则共存。

        匹配逻辑是：
            具体协议优先于 all
            https 规则只匹配 https
            http 规则只匹配 http
            all 作为兜底
        所以实际上：
            https 请求 → 走 HTTPS_PROXY
            http 请求 → 走 HTTP_PROXY
            其它协议 → 走 ALL_PROXY
        从“设计语义”角度：
        通常我们希望逻辑是：
            1️⃣ ALL_PROXY 作为默认兜底
            2️⃣ HTTP_PROXY 覆盖 http
            3️⃣ HTTPS_PROXY 覆盖 https

        
        注意：
        如果想使用Proxy::system()这是不对的，因为没有system这种类型，
        如果想自动获取系统代理(有的系统上可以，有的系统上不可以)，
        则可以使用Client::new()的默认方式创建client,让其全部默认(可能可以自动使用系统代理，跟reqwest版本相关)
        如何真正“自动读取系统代理”:
        let client = reqwest::Client::new();
        或者：
        let client = reqwest::Client::builder().build()?;

        反之则使用Client::builder()的方式手动管理，即当前下面实现的方法。
        */


        // 1️⃣ 添加 system 代理

        //这种方法是：自动读取系统代理，但是不同的系统可能会有不同的限制，如Windows下无法获取socks5代理方式，一般在GUI中使用
        // Client::builder().proxy(Proxy::system())
        // 不支持Proxy::system
        // client = client.proxy(Proxy::system());
            

        // 2️⃣ 再添加手动环境变量代理（作为 fallback）

        //这种方式是根据环境变量中的all_proxy|http_proxy|https_proxy变量的值来手动设置代理的
        //因为 reqwest 允许多个 proxy 规则共存。
        if let Ok(proxy_url) = std::env::var("https_proxy")
            .or_else(|_| std::env::var("HTTPS_PROXY"))
        {
            if let Ok(proxy) = Proxy::https(&proxy_url) {
                client = client.proxy(proxy);
            }
        }
        if let Ok(proxy_url) = std::env::var("http_proxy")
            .or_else(|_| std::env::var("HTTP_PROXY"))
        {
            if let Ok(proxy) = Proxy::http(&proxy_url) {
                client = client.proxy(proxy);
            }
        }
        if let Ok(proxy_url) = std::env::var("all_proxy")
            .or_else(|_| std::env::var("ALL_PROXY"))
        {
            if let Ok(proxy) = Proxy::all(&proxy_url) {
                client = client.proxy(proxy);
            }
        }


        // 设置超时
        // 否则代理挂掉时可能卡很久。
        // client = client
        // .connect_timeout(std::time::Duration::from_secs(10))
        // .timeout(std::time::Duration::from_secs(30));


        /*
        方法           是否 panic            是否打印错误信息
        unwrap()         是                 打印默认 panic 信息
        expect()         是                 打印你提供的错误信息
        client.build().unwrap()
        client.build().expect("Failed to build HTTP client")
        */
        client.build().unwrap()
    }


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
            // .header(COOKIE, &self.cookie[..])
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
        // // 检测文件大小 > 0 才跳过，避免空文件和上次下载中断
        // if let Ok(metadata) = std::fs::metadata(&fname) {
        //     if metadata.len() > 0 {
        //         println!("File already exists: {}, skipping download.", fname.display());
        //         return Ok(());
        //     }
        // }


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
