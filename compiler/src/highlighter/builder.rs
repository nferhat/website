//! # `builder`
//!
//! The grammars builder. This is code mostly taken from Helix.
//! Credits to them!

use eyre::{Context, Result, bail, eyre};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tempfile::TempPath;
use tokio::fs;
use tokio::process::Command;
use tracing::debug;

use super::config;

const BUILD_TARGET: &str = env!("BUILD_TARGET");

/// Builds a grammar by compiling it from source.
///
/// This function checks out the grammar repository, builds the parser,
/// and stores the compiled result in the build directory.
pub async fn build_grammar(grammar_id: &str, languages_dir: &Path) -> Result<config::Grammar> {
    let grammar_dir = languages_dir.join(grammar_id);
    if !grammar_dir.exists() {
        bail!("grammar dir does not exist")
    }

    let config_file_path = grammar_dir.join("config.toml");
    if !config_file_path.is_file() {
        bail!("language config path does not exist")
    }

    let config_file_contents = fs::read_to_string(&config_file_path)
        .await
        .context("failed to read language config file path")?;
    let config: config::Grammar =
        toml::de::from_str(&config_file_contents).context("invalid language config")?;

    // FIXME: Refetch if newer/whatnot.
    let build_dir = languages_dir.join("build");
    _ = fs::create_dir_all(&build_dir).await?;
    let parser_dir = build_dir.join(grammar_id);

    debug!(%grammar_id, "fetching grammar");
    fetch_grammar(&config, &parser_dir)
        .await
        .context("failed to fetch grammar")?;

    debug!(%grammar_id, "building grammar");
    let src_dir = parser_dir.join("src");
    let res = build_tree_sitter_library(&src_dir, &build_dir, &grammar_id, None)
        .await
        .context("failed to build grammar")?;
    match res {
        BuildStatus::AlreadyBuilt => debug!(?grammar_id, "using pre-compiled grammar"),
        BuildStatus::Built => info!(?grammar_id, "built grammar"),
    }

    Ok(config)
}

/// Fetches the grammar source code from the repository at the specified revision.
///
/// This function clones the repository and checks out the specific revision.
/// If the repository already exists and is at the correct revision, it skips fetching.
async fn fetch_grammar(config: &config::Grammar, out_dir: impl AsRef<Path>) -> Result<()> {
    let out_dir = out_dir.as_ref().to_path_buf();
    let repo_url = config.repo.clone();
    let rev = config.rev.clone();

    // Check if the directory exists and if the current revision matches
    if out_dir.exists() {
        match git(&out_dir, &["rev-parse", "HEAD"]).await {
            Ok(current_rev) => {
                // Try to resolve the desired revision to a commit hash
                match git(&out_dir, &["rev-parse", &rev]).await {
                    Ok(desired_rev) => {
                        if current_rev == desired_rev {
                            debug!(%rev, "grammar already at correct revision, skipping fetch");
                            return Ok(());
                        }
                    }
                    Err(_) => {
                        // Desired revision doesn't exist in the repo, need to refetch
                    }
                }
            }
            Err(_) => {
                // Failed to get current revision, likely not a git repo, remove and refetch
            }
        }

        // Revision doesn't match or it's not a valid git repo, remove the directory
        fs::remove_dir_all(&out_dir)
            .await
            .context("failed to remove outdated grammar directory")?;
    }

    // Clone the repository
    git(
        Path::new("."),
        &["clone", &repo_url, &out_dir.to_string_lossy()],
    )
    .await
    .context("failed to clone repository")?;

    // Checkout the specific revision
    git(&out_dir, &["checkout", &rev])
        .await
        .context("failed to checkout revision")?;

    Ok(())
}

// A wrapper around 'git' commands which returns stdout in success and a
// helpful error message showing the command, stdout, and stderr in error.
async fn git<I, S>(repository_dir: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(repository_dir)
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        // TODO: figure out how to display the git command using `args`
        Err(eyre!(
            "Git command failed.\nStdout: {}\nStderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

enum BuildStatus {
    AlreadyBuilt,
    Built,
}

async fn build_tree_sitter_library(
    src_path: &Path,
    build_dir: &Path,
    grammar_id: &str,
    target: Option<&str>,
) -> Result<BuildStatus> {
    let header_path = src_path;
    let parser_path = src_path.join("parser.c");
    let mut scanner_path = src_path.join("scanner.c");

    let scanner_path = if scanner_path.exists() {
        Some(scanner_path)
    } else {
        scanner_path.set_extension("cc");
        if scanner_path.exists() {
            Some(scanner_path)
        } else {
            None
        }
    };

    trace!(
        ?grammar_id,
        ?parser_path,
        ?scanner_path,
        "building grammar from sources"
    );

    let library_path = build_dir.join(&grammar_id).with_extension("so");

    // if we are running inside a buildscript emit cargo metadata
    // to detect if we are running from a buildscript check some env variables
    // that cargo only sets for build scripts
    if std::env::var("OUT_DIR").is_ok() && std::env::var("CARGO").is_ok() {
        if let Some(scanner_path) = scanner_path.as_ref().and_then(|path| path.to_str()) {
            println!("cargo:rerun-if-changed={scanner_path}");
        }
        if let Some(parser_path) = parser_path.to_str() {
            println!("cargo:rerun-if-changed={parser_path}");
        }
    }

    let recompile = needs_recompile(&library_path, &parser_path, scanner_path.as_ref())
        .await
        .context("failed to compare source and binary timestamps")?;

    if !recompile {
        return Ok(BuildStatus::AlreadyBuilt);
    }

    let mut config = cc::Build::new();
    config
        .cpp(true)
        .opt_level(3)
        .cargo_metadata(false)
        .host(BUILD_TARGET)
        .target(target.unwrap_or(BUILD_TARGET));
    let compiler = config.get_compiler();
    let mut command = Command::new(compiler.path());
    command.current_dir(src_path);
    for (key, value) in compiler.env() {
        command.env(key, value);
    }

    command.args(compiler.args());
    // used to delay dropping the temporary object file until after the compilation is complete
    let _path_guard;

    if compiler.is_like_msvc() {
        command
            .args(["/nologo", "/LD", "/I"])
            .arg(header_path)
            .arg("/utf-8")
            .arg("/std:c11");
        if let Some(scanner_path) = scanner_path.as_ref() {
            if scanner_path.extension() == Some("c".as_ref()) {
                command.arg(scanner_path);
            } else {
                let mut cpp_command = Command::new(compiler.path());
                cpp_command.current_dir(src_path);
                for (key, value) in compiler.env() {
                    cpp_command.env(key, value);
                }
                cpp_command.args(compiler.args());
                let object_file =
                    library_path.with_file_name(format!("{}_scanner.obj", &grammar_id));
                cpp_command
                    .args(["/nologo", "/LD", "/I"])
                    .arg(header_path)
                    .arg("/utf-8")
                    .arg("/std:c++14")
                    .arg(format!("/Fo{}", object_file.display()))
                    .arg("/c")
                    .arg(scanner_path);
                let output = cpp_command
                    .output()
                    .await
                    .context("Failed to execute C++ compiler")?;

                if !output.status.success() {
                    bail!(
                        "Parser compilation failed.\nStdout: {}\nStderr: {}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                command.arg(&object_file);
                _path_guard = TempPath::try_from_path(object_file).unwrap();
            }
        }

        command
            .arg(parser_path)
            .arg("/link")
            .arg(format!("/out:{}", library_path.to_str().unwrap()));
    } else {
        #[cfg(not(windows))]
        command.arg("-fPIC");

        command
            .arg("-shared")
            .arg("-fno-exceptions")
            .arg("-I")
            .arg(header_path)
            .arg("-o")
            .arg(&library_path);

        if let Some(scanner_path) = scanner_path.as_ref() {
            if scanner_path.extension() == Some("c".as_ref()) {
                command.arg("-xc").arg("-std=c11").arg(scanner_path);
            } else {
                let mut cpp_command = Command::new(compiler.path());
                cpp_command.current_dir(src_path);
                for (key, value) in compiler.env() {
                    cpp_command.env(key, value);
                }
                cpp_command.args(compiler.args());
                let object_file = library_path.with_file_name(format!("{}_scanner.o", &grammar_id));

                #[cfg(not(windows))]
                cpp_command.arg("-fPIC");

                cpp_command
                    .arg("-fno-exceptions")
                    .arg("-I")
                    .arg(header_path)
                    .arg("-o")
                    .arg(&object_file)
                    .arg("-std=c++14")
                    .arg("-c")
                    .arg(scanner_path);
                let output = cpp_command
                    .output()
                    .await
                    .context("Failed to execute C++ compiler")?;
                if !output.status.success() {
                    bail!(
                        "Parser compilation failed.\nStdout: {}\nStderr: {}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                command.arg(&object_file);
                _path_guard = TempPath::try_from_path(object_file).unwrap();
            }
        }
        command.arg("-xc").arg("-std=c11").arg(parser_path);
        if cfg!(all(
            unix,
            not(any(target_os = "macos", target_os = "illumos"))
        )) {
            command.arg("-Wl,-z,relro,-z,now");
        }
    }

    let output = command
        .output()
        .await
        .context("Failed to execute C/C++ compiler")?;
    if !output.status.success() {
        bail!(
            "Parser compilation failed.\nStdout: {}\nStderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(BuildStatus::Built)
}

async fn needs_recompile(
    lib_path: &Path,
    parser_c_path: &Path,
    scanner_path: Option<&PathBuf>,
) -> Result<bool> {
    if !lib_path.exists() {
        return Ok(true);
    }
    let lib_mtime = mtime(lib_path).await?;
    if mtime(parser_c_path).await? > lib_mtime {
        return Ok(true);
    }
    if let Some(scanner_path) = scanner_path {
        if mtime(scanner_path).await? > lib_mtime {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn mtime(path: &Path) -> Result<SystemTime> {
    Ok(fs::metadata(path).await?.modified()?)
}
