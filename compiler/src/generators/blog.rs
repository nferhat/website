pub fn prelude(title: impl AsRef<str>, stylesheet_path: Option<&str>) -> String {
    let mut stylesheet_include = String::new();
    if let Some(path) = stylesheet_path {
        stylesheet_include = format!(r#"<link rel="stylesheet" href="{path}" />"#);
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
    </head>

    <body><div class="content">
"#
    )
}

pub fn epilogue() -> String {
    String::from("</div></body></html>")
}
