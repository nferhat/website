static MATHJAX_SCRIPT_URL: &'static str = "https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js";

pub fn prelude(
    title: impl AsRef<str>,
    stylesheet_path: Option<&str>,
    include_math_script: bool,
) -> String {
    let mut stylesheet_include = String::new();
    if let Some(path) = stylesheet_path {
        stylesheet_include = format!(r#"<link rel="stylesheet" href="{path}" />"#);
    }

    let mut math_script = String::new();
    if include_math_script {
        math_script = format!(
            r#"
            <script> MathJax = {{ options: {{ enableMenu: false }} }}; </script>
            <script defer id="mathjax" src="{MATHJAX_SCRIPT_URL}"></script>"#
        );
    }

    let title = title.as_ref();
    format!(
        r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>{title}</title>
        {stylesheet_include}
        {math_script}
    </head>

    <body><div class="content">
"#
    )
}

pub fn epilogue() -> String {
    String::from("</div></body></html>")
}
