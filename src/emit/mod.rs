pub mod dot;
pub mod mermaid;
pub mod unicode;

use crate::schema::Schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Unicode,
    Ascii,
    Dot,
    Mermaid,
}

pub fn render(format: Format, schema: &Schema) -> String {
    match format {
        Format::Unicode => unicode::render(schema, false),
        Format::Ascii => unicode::render(schema, true),
        Format::Dot => dot::render(schema),
        Format::Mermaid => mermaid::render(schema),
    }
}
