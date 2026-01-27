use reqwest::header::*;
use std::path::Path;
use std::fmt;

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

#[derive(Clone)]
pub struct Handler {
    pub client: reqwest::Client,
    pub host: String,
    cookie: String,
}

impl Handler {
    pub fn new(host: &str, cookie: &str) -> Self {
        Handler {
            client: reqwest::Client::new(),
            host: host.to_string(),
            cookie: cookie.to_string(),
        }
    }

    pub fn request(&self, task: &str, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        let res = self
            .client
            .get(url)
            .header(COOKIE, &self.cookie[..])
            .header(HOST, &self.host[..])
            .header(
                USER_AGENT,
                "Mozilla/5.0 (X11; Linux x86_64; rv:65.0) Gecko/20100101 Firefox/65.0",
            )
            .send();
        res
    }

    pub fn download(target: &str, path: &str, filename: &str, cookie: &str) -> Result<(), DownloadError> {
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

        let fname = Path::new(path).join(filename);
        let mut dest = match std::fs::File::create(&fname) {
            Ok(f) => f,
            Err(e) => return Err(DownloadError::Io(e)),
        };

        println!("Downloading: {}", fname.to_str().unwrap());
        if let Err(e) = std::io::copy(&mut res, &mut dest) {
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
