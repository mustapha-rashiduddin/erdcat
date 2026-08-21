use crate::schema::{Schema, Table};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn id(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn port(col: &str) -> String {
    let safe: String = col
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("col_{safe}")
}

fn node_port(schema: &Schema, table: &str, col: &str) -> String {
    match schema.tables.get(table) {
        Some(t) if t.columns.iter().any(|c| c.name.eq_ignore_ascii_case(col)) => {
            format!(":{}", port(col))
        }
        _ => String::new(),
    }
}

fn node(table: &Table) -> String {
    let mut s = format!(
        "  {} [label=<<table border=\"1\" cellborder=\"1\" cellspacing=\"0\">",
        id(&table.name)
    );
    s.push_str(&format!("<tr><td><b>{}</b></td></tr>", esc(&table.name)));
    for c in &table.columns {
        let mut cell = if c.primary_key {
            format!("<b>{}</b>", esc(&c.name))
        } else {
            esc(&c.name)
        };
        let labels = table.column_key_labels(c);
        if !labels.is_empty() {
            cell.push(' ');
            cell.push_str(&labels.join(", "));
        }
        if !c.data_type.is_empty() {
            cell.push_str(" : ");
            cell.push_str(&esc(&c.data_type));
        }
        s.push_str(&format!(
            "<tr><td port=\"{}\">{}</td></tr>",
            port(&c.name),
            cell
        ));
    }
    s.push_str("</table>>];");
    s
}

pub fn render(schema: &Schema) -> String {
    let junction_names = schema.collapsible_junctions();

    let mut out = String::from("digraph erd {\n  rankdir=LR;\n  node [shape=plaintext];\n");

    for t in schema.tables.values() {
        if !junction_names.contains(&t.name) {
            out.push_str(&node(t));
            out.push('\n');
        }
    }

    for t in schema.tables.values() {
        if junction_names.contains(&t.name) {
            continue;
        }
        for fk in &t.foreign_keys {
            if junction_names.contains(&fk.to_table) {
                continue;
            }
            out.push_str(&format!(
                "  {}{} -> {}{} [taillabel=\"*\", headlabel=\"1\"];\n",
                id(&t.name),
                node_port(schema, &t.name, &fk.from_column),
                id(&fk.to_table),
                node_port(schema, &fk.to_table, &fk.to_column),
            ));
        }
    }

    for t in schema.tables.values() {
        if !junction_names.contains(&t.name) {
            continue;
        }
        if let Some((a, b)) = t.junction_foreign_keys() {
            out.push_str(&format!(
                "  {}{} -> {}{} [dir=both, arrowtail=crow, arrowhead=crow];\n",
                id(&a.to_table),
                node_port(schema, &a.to_table, &a.to_column),
                id(&b.to_table),
                node_port(schema, &b.to_table, &b.to_column),
            ));
        }
    }

    out.push_str("}\n");
    out
}
