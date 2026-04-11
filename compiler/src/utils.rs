use markdown::mdast::Node;

/// Counts all the words inside of a given node. You should pass in the root node
/// here to get all the words inside a document.
pub fn count_words(node: &Node) -> usize {
    match node {
        Node::Text(text) => count_words_in_text(&text.value),
        Node::InlineCode(code) => count_words_in_text(&code.value),
        Node::Root(n) => n.children.iter().map(count_words).sum(),
        Node::Paragraph(n) => n.children.iter().map(count_words).sum(),
        Node::Heading(n) => n.children.iter().map(count_words).sum(),
        Node::Strong(n) => n.children.iter().map(count_words).sum(),
        Node::Emphasis(n) => n.children.iter().map(count_words).sum(),
        Node::Delete(n) => n.children.iter().map(count_words).sum(),
        Node::Link(n) => n.children.iter().map(count_words).sum(),
        Node::LinkReference(n) => n.children.iter().map(count_words).sum(),
        Node::Blockquote(n) => n.children.iter().map(count_words).sum(),
        Node::List(n) => n.children.iter().map(count_words).sum(),
        Node::ListItem(n) => n.children.iter().map(count_words).sum(),
        Node::Table(n) => n.children.iter().map(count_words).sum(),
        Node::TableRow(n) => n.children.iter().map(count_words).sum(),
        Node::TableCell(n) => n.children.iter().map(count_words).sum(),

        // not including code blocks? I dont know if this is really correct but eh
        _ => 0,
    }
}

fn count_words_in_text(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Estimate the reading time in `(minutes, seconds)` with a given word-per-minute
/// reading speed. You should adapt this on the blog itself
pub fn estimate_reading_time_min_sec(word_count: usize, wpm: f64) -> (usize, usize) {
    let total_seconds = ((word_count as f64 / wpm) * 60.0).round() as usize;
    (total_seconds / 60, total_seconds % 60)
}
