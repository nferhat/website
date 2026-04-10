pub fn prelude(title: impl AsRef<str>) -> String {
    let title = title.as_ref();
    format!(
        r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>{title}</title>
        <!-- FIXME: Styling using compiled SASS -->
        <style>
            :root {{
                background-color: #141417;
                color: #cecece;
            }}

            .codeblock {{
                position: relative;
            }}
            .language-name {{
                font-family: monospace;
                position: absolute;
                padding: 6px;
                background-color: #cecece09;
                border-radius: 4px;
                right: 0;
                margin: 8px;
            }}

            pre {{
                padding: 12px;
                background-color: #101012;
                border-radius: 10px;
            }}

            .content {{
                margin-left: 30vw;
                margin-right: 10vw;
            }}
        </style>
    </head>

    <body><div class="content">
"#
    )
}

pub fn epilogue() -> String {
    String::from("</div></body></html>")
}
