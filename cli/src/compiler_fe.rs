//! compiler_fe.rs -*- compiler **f**ront **e**nd.
//! Just some helper functions to compile files down, so that other parts of the code dont have to touch it.

use anyhow::bail;
use compiler::page::SitePage;
use compiler::{Config, Generator, StylingConfig, frontmatter};
use markdown::mdast;
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::task::spawn_blocking;

use anyhow::Context;

pub async fn compile_styles(
    style_input: impl AsRef<Path>,
    config: &StylingConfig,
) -> anyhow::Result<String> {
    let style_input = style_input.as_ref();
    // include the file's parent directory in the import paths
    let import_path = style_input.parent().context("missing parent")?.to_owned();
    let sass_input = fs::read_to_string(&style_input).await?;
    // and also what the user wants us to import
    let mut load_paths = config.load_paths.clone();
    load_paths.push(import_path);

    // asyncify the compiling process, since it might read from other files on the filesystem and such
    // I don't know if this even helps but who am I to talk
    let style_content =
        match spawn_blocking(move || compiler::compile_to_stylesheet(&sass_input, &load_paths))
            .await
        {
            Ok(res) => res.context("failed to parse css"),
            Err(_) => Err(anyhow::anyhow!("background task failed")),
        };
    let style_content = style_content.context("failed to compile stylesheet")?;
    Ok(style_content)
}

pub async fn compile_file(
    input_path: impl AsRef<Path>,
    _config: &Config,
    dev: bool,
) -> anyhow::Result<String> {
    let input_path = input_path.as_ref();
    let name = input_path.file_name().unwrap();
    let raw_content = fs::read_to_string(input_path).await?;
    let mut contents = markdown::to_mdast(&raw_content, &parse_options())
        .map_err(|msg| anyhow::anyhow!("failed to parse markdown: {msg:?}"))?;

    // Get the frontmatter out.
    let children = contents.children_mut().context("missing content")?;
    if children.len() < 1 {
        bail!("file without frontmatter");
    }
    let frontmatter_node = children.remove(0);
    let mdast::Node::Toml(mdast::Toml {
        value: frontmatter_str,
        ..
    }) = frontmatter_node
    else {
        anyhow::bail!("file with invalid frontmatter");
    };

    let frontmatter = frontmatter::from_str(&frontmatter_str)?;

    let page = SitePage {
        name: name.to_string_lossy().to_string(),
        path: input_path.to_owned(),
        frontmatter,
        contents,
    };

    let generator = Generator::new(page, Some("/style.css"), dev);
    let html = generator.html()?;
    Ok(html)
}

/// Options used to do the parsing of the config.
///
/// It's GFM+some additional stuff enabled.
fn parse_options() -> markdown::ParseOptions {
    markdown::ParseOptions {
        constructs: markdown::Constructs {
            autolink: true,
            code_text: true,
            gfm_autolink_literal: true,
            html_flow: true,
            label_start_image: true,
            math_flow: true,
            block_quote: true,
            frontmatter: true,
            math_text: true,
            ..markdown::Constructs::gfm()
        },
        ..Default::default()
    }
}

pub async fn build_all(
    root: &PathBuf,
    build_dir: &PathBuf,
    config: &Config,
    dev: bool,
) -> anyhow::Result<()> {
    // We gotta make it absolute for things to work here.
    // Notably, we use Path::strip_prefix in order to correctly calculate the build directories
    let root = if root.is_absolute() {
        root.to_owned()
    } else {
        env::current_dir().expect("no cwd").join(root)
    };

    let style_input = root.join(&config.styling.root_file);
    let contents = compile_styles(&style_input, &config.styling)
        .await
        .context("failed to build stylesheets")?;

    let output_path = build_dir.join("style.css");
    fs::write(&output_path, contents)
        .await
        .context("failed to write stylesheets")?;

    info!("Built stylesheets");

    let pages = compiler::page::get_pages(&root).context("failed to get pages")?;
    info!(path = ?root, "Got {} pages in path", pages.len());

    for page in pages {
        let path_relative = page.path.strip_prefix(&root).expect("relative to root");
        let output_path = build_dir.join(path_relative).with_extension("html");

        info!(source = ?page.path, output = ?output_path, "Generating page");
        fs::create_dir_all(output_path.parent().unwrap()).await?;

        // Do not include hot reload script if we are building the final distribution content
        let html = compile_file(&page.path, config, dev).await?;
        if let Err(err) = fs::write(&output_path, html).await {
            bail!("Failed to generate page: {err:?}");
        }
    }

    Ok(())
}
