use std::fs;

use compiler::parse_content_dir;
use markdown::mdast;

fn main() {
    let path = std::path::PathBuf::from("test/content");
    let res = parse_content_dir(&path).unwrap();

    _ = fs::create_dir("dist/");
    for page in res {
        let ast = page.contents;
        print_code_contents(&ast);
    }
}

fn print_code_contents(node: &mdast::Node) {
    let Some(children) = node.children() else {
        return;
    };

    for child in children {
        match child {
            mdast::Node::Code(mdast::Code { value, lang, .. }) => {
                println!("Got code with lang={lang:?}");
                dbg!(value);
            }

            node => print_code_contents(node),
        }
    }
}
