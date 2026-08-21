use crate::schema::Schema;

fn ident(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    match cleaned.chars().next() {
        Some(c) if c.is_ascii_digit() => format!("_{cleaned}"),
        Some(_) => cleaned,
        None => "empty".to_string(),
    }
}

pub fn render(schema: &Schema) -> String {
    let mut out = String::from("erDiagram\n");

    for t in schema.tables.values() {
        if t.junction_targets().is_some() {
            continue;
        }
        out.push_str(&format!("    {} {{\n", ident(&t.name)));
        for c in &t.columns {
            let ty = ident(if c.data_type.is_empty() {
                "TEXT"
            } else {
                &c.data_type
            });
            out.push_str(&format!("        {} {}", ty, ident(&c.name)));
            if c.primary_key {
                out.push_str(" PK");
            } else if c.not_null {
                out.push_str(" \"NOT NULL\"");
            }
            out.push('\n');
        }
        out.push_str("    }\n");
    }

    for t in schema.tables.values() {
        if let Some((a, b)) = t.junction_targets() {
            out.push_str(&format!("    {} ||--o{{ {}\n", ident(&a), ident(&b)));
            continue;
        }
        for fk in &t.foreign_keys {
            out.push_str(&format!(
                "    {} ||--o{{ {} : {}\n",
                ident(&fk.to_table),
                ident(&t.name),
                ident(&fk.from_column)
            ));
        }
    }

    out
}
