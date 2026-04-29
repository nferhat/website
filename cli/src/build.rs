//! # `build` - final build for website
//!
//! This generates the final website when calling `cli build`.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use compiler::Compiler;
use compiler::fonts;
use eyre::Context;
use eyre::Result;
use eyre::bail;
use futures::future::join_all;
use tokio::fs;
use tokio::io;

pub(crate) async fn compile(mut compiler: Compiler) -> Result<()> {
    compiler.compile_all().await?;

    // After compiling, include the static directory.
    //
    // FIXME: There's a case where if the fonts are also in the static directory, and are also mentionned by bundle-fonts,
    // it would serve non-subsetted fonts in local (cli serve) but subsetted fonts in prod. This isn't a huge huge issue
    // but still.
    let build_path = compiler.build_path();
    dircpy::copy_dir("static", build_path.join("static"))
        .context("failed to copy static contents")?;
    // Now we have todo font subsetting.
    subset_fonts(&compiler)
        .await
        .context("failed to subset fonts")?;

    Ok(())
}

async fn subset_fonts(compiler: &Compiler) -> Result<()> {
    // First collect all the characters.
    // FIXME: We also include HTMl characters but this shouldn't hurt too much.
    let mut all_chars = HashSet::new();
    for page in compiler.pages() {
        let path = PathBuf::from(&page.output_path);
        let contents = fs::read_to_string(&path).await?;
        all_chars.extend(contents.chars());
    }
    let all_chars = Arc::new(all_chars);

    // Then subset on that.
    // Run the font builds in parallel, since subsetting takes quite a bit of time to process.
    let bundle_fonts = compiler.config().styling.bundle_fonts.clone();
    let len = bundle_fonts.len();

    let mut jobs = Vec::with_capacity(len);
    for path in &bundle_fonts {
        let all_chars = Arc::clone(&all_chars);
        let path = path.clone();
        jobs.push(tokio::task::spawn_blocking(move || {
            match subset_font(&path, &*all_chars) {
                Ok((style, bytes)) => Some((path, style, bytes)),
                Err(err) => {
                    warn!(?err, "Failed to subset font");
                    None
                }
            }
        }));
    }

    // We need the static directory ready here.
    let fonts_build_path = compiler.build_path().join("static").join("fonts");
    _ = fs::create_dir(&fonts_build_path).await;

    for result in join_all(jobs).await {
        let Ok(Some((path, font_style, bytes))) = result else {
            continue;
        };

        let filename = path.file_name().unwrap().to_string_lossy();
        let out_path = fonts_build_path.join(&*filename);

        match fs::remove_file(&out_path).await {
            // HACK: Case where the font is also inside the static directory.
            Ok(()) => (),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => (),
            Err(err) => bail!(err),
        }

        fs::write(&out_path, bytes).await?;
        info!(
            family = font_style.family,
            import_url = format!("/static/fonts/{filename}"),
            weight = font_style.weight,
            style = if font_style.is_bold {
                "bold"
            } else if font_style.is_italic {
                "italic"
            } else {
                "normal"
            },
            "Subsetted font"
        )
    }

    Ok(())
}

fn subset_font(font_path: &Path, all_chars: &HashSet<char>) -> Result<(fonts::Style, Vec<u8>)> {
    let font_bytes = std::fs::read(&font_path).context("failed to read font bytes")?;
    let result_bytes =
        fonts::subset(&font_bytes, all_chars.iter().copied()).context("failed to subset font")?;
    let style = fonts::font_style(&font_bytes).context("failed to get font style info")?;

    Ok((style, result_bytes))
}
