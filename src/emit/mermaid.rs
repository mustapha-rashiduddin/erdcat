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
    let junction_names = schema.collapsible_junctions();

    for t in schema.tables.values() {
        if junction_names.contains(&t.name) {
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
            let foreign_key = t.is_foreign_key_column(c);
            match (c.primary_key, foreign_key) {
                (true, true) => out.push_str(" PK, FK"),
                (true, false) => out.push_str(" PK"),
                (false, true) => out.push_str(" FK"),
                (false, false) => {}
            }
            let labels = t.column_key_labels(c);
            let mut details = Vec::new();
            if labels.iter().any(|label| label != "PK" && label != "FK") {
                details.push(labels.join(", "));
            }
            if !c.primary_key && c.not_null {
                details.push("NOT NULL".to_string());
            }
            if !details.is_empty() {
                out.push_str(&format!(" \"{}\"", details.join("; ")));
            }
            out.push('\n');
        }
        out.push_str("    }\n");
    }

    for t in schema.tables.values() {
        if junction_names.contains(&t.name) {
            let (a, b) = t
                .junction_foreign_keys()
                .expect("collapsible junction has two foreign keys");
            out.push_str(&format!(
                "    {} ||--o{{ {}\n",
                ident(&a.to_table),
                ident(&b.to_table)
            ));
            continue;
        }
        for fk in &t.foreign_keys {
            if junction_names.contains(&fk.to_table) {
                continue;
            }
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
