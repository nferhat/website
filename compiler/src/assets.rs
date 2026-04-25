//! # `assets` - Asset handling.
//!
//! When a file includes an asset, we look it up in the content system, get its content hash,
//! cache it and put it inside the build directory as `/static/<content-hash>.<extension>`
//!
//! Doing so will allow us to aggresively cache the content, so that browsers don't
//! have to reload it, unless it changes. For the writer perspective, you just include some file in
//! your markdown, and that's it really.

use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};

use blake3::hazmat::HasherExt;
use eyre::{Result, bail, eyre};
use tokio::sync::Mutex;
use tokio::{fs, io::AsyncReadExt as _};

pub struct AssetRegistry {
    asset_base_path: PathBuf,
    build_dir: PathBuf,
    /// Cached assets, where `AssetKey` is the key generated from the asset path and the value
    /// is the actual asset URL from the server.
    cache: Mutex<HashMap<PathBuf, [u8; 32]>>,
}

impl AssetRegistry {
    pub fn new(asset_base_path: PathBuf, build_dir: PathBuf) -> Self {
        Self {
            asset_base_path,
            build_dir,
            cache: Default::default(),
        }
    }

    /// Tries to load a given asset from it's `asset_path`. Relative paths (for example `./cat.png`) will be
    /// relative to the given `source_path`. Returns the final URL to load the asset from.
    pub async fn load(&self, asset_path_str: &str, source_path: &Path) -> Result<String> {
        let source_path = source_path.parent().ok_or(eyre!("no parent"))?;
        let asset_path = PathBuf::from(asset_path_str);
        let extension = asset_path
            .extension()
            .ok_or(eyre!("missing extension"))?
            .to_string_lossy();

        // First case: We are trying to load an asset relative to the `source_path`
        let asset_path_real = if asset_path_str.starts_with("./") {
            let asset_path_real = source_path.join(&asset_path);
            if !asset_path_real.is_file() {
                bail!("invalid asset '{asset_path:?}'");
            }

            asset_path_real
        } else if asset_path_str.starts_with("/") {
            let asset_path_real = self.asset_base_path.join(&asset_path_str[1..]);
            if !asset_path_real.is_file() {
                bail!("invalid asset '{asset_path:?}'");
            }

            asset_path_real
        } else {
            bail!("invalid asset '{asset_path:?}'");
        };

        let mut cache = self.cache.lock().await;
        let mut filename = String::new();

        if let Some(content_hash) = cache.get(&asset_path_real) {
            for byte in content_hash {
                write!(&mut filename, "{byte:02x}")?;
            }
            write!(&mut filename, ".{extension}")?;
            Ok(format!("/static/{filename}"))
        } else {
            // Note inside the cache, read and insert.
            let file = fs::File::open(&asset_path_real).await?;

            let content_hash = content_hash(file).await?;
            for byte in &content_hash {
                write!(&mut filename, "{byte:02x}")?;
            }

            debug!(path = %asset_path_str, content_hash = %filename, "Caching asset");
            cache.insert(asset_path_real.clone(), content_hash);

            write!(&mut filename, ".{extension}")?;

            // Now copy the file over.
            let output_path_final = self.build_dir.join("static").join(&filename);
            fs::copy(&asset_path_real, &output_path_final).await?;
            Ok(format!("/static/{filename}"))
        }
    }

    pub async fn try_reload_asset(&mut self, path: &Path) -> Result<(bool, bool)> {
        let Some(extension) = path.extension().map(ToOwned::to_owned) else {
            // can't do much without an extension m8
            return Ok((false, false));
        };
        let extension = extension.to_string_lossy();

        let path_str = path.to_string_lossy();
        trace!(path = %path_str, "Trying to reload asset");
        let path = path.to_path_buf();
        let cache = self.cache.get_mut();
        let Some(previous_content_hash) = cache.remove(&path) else {
            trace!(path = %path_str, "Asset not in registry");
            return Ok((false, false)); // no an asset.
        };

        // Now do the same thing as `load` but compare caches and return if we need to rebuild.
        let file = fs::File::open(&path).await?;
        let mut filename = String::new();

        let content_hash = content_hash(file).await?;
        // If content hash matches no need to rebuild the site
        if content_hash == previous_content_hash {
            debug!(path = %path_str, "Skipping asset reloading since content hash matches");
            return Ok((true, false));
        }

        for byte in &content_hash {
            write!(&mut filename, "{byte:02x}")?;
        }

        debug!(path = %path_str, content_hash = %filename, "Caching asset");
        cache.insert(path.clone(), content_hash);

        write!(&mut filename, ".{extension}")?;

        // FIXME: For now I don't have any way to invalidate the cache.

        // Now copy the file over.
        let output_path_final = self.build_dir.join("static").join(&filename);
        fs::copy(&path, &output_path_final).await?;
        Ok((true, true))
    }
}

async fn content_hash(mut file: fs::File) -> Result<[u8; 32]> {
    let mut content_hasher = blake3::Hasher::new();
    let mut buf = [0u8; 8192];

    loop {
        let n = file.read(&mut buf).await?;
        if n != 0 {
            content_hasher.update(&buf);
            continue;
        }

        break;
    }

    Ok(content_hasher.finalize_non_root())
}
