use render::html;

pub fn generate_code_html(code: String, lang: Option<String>) -> String {
    return html! {
            <code>
                <pre>
    {code}
                </pre>
            </code>
        };
}
