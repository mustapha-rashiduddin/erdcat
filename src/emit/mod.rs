pub mod dot;
pub mod mermaid;

use crate::schema::Schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Dot,
    Mermaid,
}

pub fn render(format: Format, schema: &Schema) -> String {
    match format {
        Format::Dot => dot::render(schema),
        Format::Mermaid => mermaid::render(schema),
    }
}
