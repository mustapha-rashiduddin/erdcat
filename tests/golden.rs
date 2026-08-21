use erdcat::emit::{self, Format};
use erdcat::schema::Schema;
use rusqlite::Connection;

fn fixture() -> Schema {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE author (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE book (id INTEGER PRIMARY KEY, title TEXT, author_id INTEGER REFERENCES author(id));
         CREATE TABLE author_book (
             author_id INTEGER REFERENCES author(id),
             book_id INTEGER REFERENCES book(id),
             PRIMARY KEY (author_id, book_id)
         );
         CREATE TABLE lonely (x INTEGER);",
    )
    .unwrap();
    Schema::load(&conn).unwrap()
}

#[test]
fn dot_renders_tables_edges_and_junctions() {
    let dot = emit::render(Format::Dot, &fixture());
    assert!(dot.starts_with("digraph erd {"));
    assert!(dot.contains("\"author\""));
    assert!(dot.contains("\"book\""));
    assert!(dot.contains("\"lonely\""));
    assert!(!dot.contains("\"author_book\""));
    assert!(dot.contains("\"book\":col_author_id -> \"author\":col_id"));
    assert!(dot.contains("arrowtail=crow"));
    let mn = dot.contains("\"book\" -> \"author\"") || dot.contains("\"author\" -> \"book\"");
    assert!(mn);
    assert!(dot.ends_with("}\n"));
}

#[test]
fn mermaid_renders_entities_and_relationships() {
    let mmd = emit::render(Format::Mermaid, &fixture());
    assert!(mmd.starts_with("erDiagram\n"));
    assert!(mmd.contains("author {"));
    assert!(mmd.contains("INTEGER id PK"));
    assert!(mmd.contains("author ||--o{ book"));
    assert!(!mmd.contains("author_book {"));
}

#[test]
fn self_reference_and_missing_target_resolve() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE node (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES node);
         CREATE TABLE orphan (id INTEGER, other_id INTEGER REFERENCES missing);",
    )
    .unwrap();
    let schema = Schema::load(&conn).unwrap();
    let dot = emit::render(Format::Dot, &schema);
    assert!(dot.contains("\"node\":col_parent_id -> \"node\":col_id"));
    assert!(dot.contains("\"orphan\":col_other_id -> \"missing\""));
}
