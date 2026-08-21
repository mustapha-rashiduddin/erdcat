use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub primary_key_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKey {
    pub id: usize,
    pub sequence: usize,
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
    fn foreign_key_constraints(&self) -> Vec<Vec<&ForeignKey>> {
        let mut constraints: BTreeMap<usize, Vec<&ForeignKey>> = BTreeMap::new();
        for fk in &self.foreign_keys {
            constraints.entry(fk.id).or_default().push(fk);
        }
        let mut constraints: Vec<_> = constraints.into_values().collect();
        for constraint in &mut constraints {
            constraint.sort_by_key(|fk| fk.sequence);
        }
        constraints
    }

    pub(crate) fn column_key_labels(&self, column: &Column) -> Vec<String> {
        let mut labels = Vec::new();
        if column.primary_key {
            let count = self
                .columns
                .iter()
                .filter(|column| column.primary_key)
                .count();
            if count == 1 {
                labels.push("PK".to_string());
            } else {
                labels.push(format!("PK({}/{count})", column.primary_key_position));
            }
        }

        let constraints = self.foreign_key_constraints();
        let constraint_count = constraints.len();
        for (index, constraint) in constraints.into_iter().enumerate() {
            let prefix = if constraint_count == 1 {
                "FK".to_string()
            } else {
                format!("FK{}", index + 1)
            };
            for fk in constraint
                .iter()
                .filter(|fk| fk.from_column.eq_ignore_ascii_case(&column.name))
            {
                if constraint.len() == 1 {
                    labels.push(prefix.clone());
                } else {
                    labels.push(format!(
                        "{prefix}({}/{})",
                        fk.sequence + 1,
                        constraint.len()
                    ));
                }
            }
        }
        labels
    }

    pub(crate) fn is_foreign_key_column(&self, column: &Column) -> bool {
        self.foreign_keys
            .iter()
            .any(|fk| fk.from_column.eq_ignore_ascii_case(&column.name))
    }

    pub fn junction_foreign_keys(&self) -> Option<(&ForeignKey, &ForeignKey)> {
        let constraints = self.foreign_key_constraints();
        if constraints.len() != 2 {
            return None;
        }
        let has_extra = self.columns.iter().any(|c| {
            !c.primary_key && !self.foreign_keys.iter().any(|fk| fk.from_column == c.name)
        });
        if has_extra {
            return None;
        }

        let a = constraints[0].first()?;
        let b = constraints[1].first()?;
        if a.to_table.eq_ignore_ascii_case(&b.to_table) {
            None
        } else {
            Some((a, b))
        }
    }

    pub fn junction_targets(&self) -> Option<(String, String)> {
        self.junction_foreign_keys()
            .map(|(a, b)| (a.to_table.clone(), b.to_table.clone()))
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
            "SELECT name FROM pragma_table_list \
             WHERE schema = 'main' \
               AND type IN ('table', 'virtual') \
               AND name NOT GLOB 'sqlite_*' \
             ORDER BY LOWER(name), name",
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
                conn.prepare(&format!("PRAGMA table_xinfo({})", quote_ident(name)))?;
            let mut cols = col_stmt.query([])?;
            while let Some(row) = cols.next()? {
                let cname: String = row.get(1)?;
                let ctype: String = row.get(2)?;
                let notnull: i64 = row.get(3)?;
                let pk: i64 = row.get(5)?;
                let hidden: i64 = row.get(6)?;
                if hidden == 1 {
                    continue;
                }
                table.columns.push(Column {
                    name: cname,
                    data_type: ctype,
                    not_null: notnull != 0,
                    primary_key: pk > 0,
                    primary_key_position: pk as usize,
                });
            }

            let mut fk_stmt =
                conn.prepare(&format!("PRAGMA foreign_key_list({})", quote_ident(name)))?;
            let mut fks = fk_stmt.query([])?;
            while let Some(row) = fks.next()? {
                let id: i64 = row.get(0)?;
                let sequence: i64 = row.get(1)?;
                let to_table: String = row.get(2)?;
                let from_column: String = row.get(3)?;
                let to_column: Option<String> = row.get(4)?;
                table.foreign_keys.push(ForeignKey {
                    id: id as usize,
                    sequence: sequence as usize,
                    from_column,
                    to_table,
                    to_column: to_column.unwrap_or_default(),
                });
            }

            schema.tables.insert(table.name.clone(), table);
        }

        let table_names: HashMap<String, String> = schema
            .tables
            .keys()
            .map(|name| (name.to_ascii_lowercase(), name.clone()))
            .collect();
        let target_columns: HashMap<String, Vec<Column>> = schema
            .tables
            .iter()
            .map(|(name, table)| (name.clone(), table.columns.clone()))
            .collect();
        for table in schema.tables.values_mut() {
            let source_columns = &table.columns;
            for fk in &mut table.foreign_keys {
                if let Some(column) = source_columns
                    .iter()
                    .find(|column| column.name.eq_ignore_ascii_case(&fk.from_column))
                {
                    fk.from_column.clone_from(&column.name);
                }
                if let Some(name) = table_names.get(&fk.to_table.to_ascii_lowercase()) {
                    fk.to_table.clone_from(name);
                }
                let Some(columns) = target_columns.get(&fk.to_table) else {
                    if fk.to_column.is_empty() {
                        fk.to_column = "rowid".to_string();
                    }
                    continue;
                };
                if fk.to_column.is_empty() {
                    fk.to_column = columns
                        .iter()
                        .find(|column| column.primary_key_position == fk.sequence + 1)
                        .or_else(|| columns.iter().find(|column| column.primary_key))
                        .map(|column| column.name.clone())
                        .unwrap_or_else(|| "rowid".to_string());
                } else if let Some(column) = columns
                    .iter()
                    .find(|column| column.name.eq_ignore_ascii_case(&fk.to_column))
                {
                    fk.to_column.clone_from(&column.name);
                }
            }
        }

        Ok(schema)
    }

    pub fn collapsible_junctions(&self) -> BTreeSet<String> {
        self.tables
            .values()
            .filter(|table| {
                let Some((a, b)) = table.junction_foreign_keys() else {
                    return false;
                };
                if !self.tables.contains_key(&a.to_table) || !self.tables.contains_key(&b.to_table)
                {
                    return false;
                }
                !self.tables.values().any(|other| {
                    other.name != table.name
                        && other
                            .foreign_keys
                            .iter()
                            .any(|fk| fk.to_table == table.name)
                })
            })
            .map(|table| table.name.clone())
            .collect()
    }
}
