use rusqlite::Connection;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

#[derive(Debug, Clone, Default)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
}

impl Table {
    pub fn junction_targets(&self) -> Option<(String, String)> {
        if self.foreign_keys.len() != 2 {
            return None;
        }
        let fk_cols: Vec<&str> = self
            .foreign_keys
            .iter()
            .map(|fk| fk.from_column.as_str())
            .collect();
        let has_extra = self
            .columns
            .iter()
            .any(|c| !c.primary_key && !fk_cols.contains(&c.name.as_str()));
        if has_extra {
            return None;
        }
        let a = &self.foreign_keys[0].to_table;
        let b = &self.foreign_keys[1].to_table;
        if a == b {
            None
        } else {
            Some((a.clone(), b.clone()))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub tables: BTreeMap<String, Table>,
}

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

impl Schema {
    pub fn open(path: &Path) -> rusqlite::Result<Schema> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Schema::load(&conn)
    }

    pub fn load(conn: &Connection) -> rusqlite::Result<Schema> {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY LOWER(name)",
        )?;
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut schema = Schema::default();
        for name in &names {
            let mut table = Table {
                name: name.clone(),
                ..Default::default()
            };

            let mut col_stmt =
                conn.prepare(&format!("PRAGMA table_info({})", quote_ident(name)))?;
            let mut cols = col_stmt.query([])?;
            while let Some(row) = cols.next()? {
                let cname: String = row.get(1)?;
                let ctype: String = row.get(2)?;
                let notnull: i64 = row.get(3)?;
                let pk: i64 = row.get(5)?;
                table.columns.push(Column {
                    name: cname,
                    data_type: ctype,
                    not_null: notnull != 0,
                    primary_key: pk > 0,
                });
            }

            let mut fk_stmt =
                conn.prepare(&format!("PRAGMA foreign_key_list({})", quote_ident(name)))?;
            let mut fks = fk_stmt.query([])?;
            while let Some(row) = fks.next()? {
                let to_table: String = row.get(2)?;
                let from_column: String = row.get(3)?;
                let to_column: Option<String> = row.get(4)?;
                table.foreign_keys.push(ForeignKey {
                    from_column,
                    to_table,
                    to_column: to_column.unwrap_or_default(),
                });
            }

            schema.tables.insert(table.name.clone(), table);
        }

        let mut fixes: Vec<(String, usize, String)> = Vec::new();
        for table in schema.tables.values() {
            for (i, fk) in table.foreign_keys.iter().enumerate() {
                if fk.to_column.is_empty() {
                    let resolved = schema
                        .tables
                        .get(&fk.to_table)
                        .and_then(|t| t.columns.iter().find(|c| c.primary_key))
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "rowid".to_string());
                    fixes.push((table.name.clone(), i, resolved));
                }
            }
        }
        for (tname, i, col) in fixes {
            if let Some(table) = schema.tables.get_mut(&tname) {
                table.foreign_keys[i].to_column = col;
            }
        }

        Ok(schema)
    }
}
