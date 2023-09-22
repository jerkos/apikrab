use crate::HOME_DIR;
use lazy_static::lazy_static;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::process::ExitStatus;

lazy_static! {
    pub static ref TABLES_NAMES: Vec<&'static str> = vec![
        "actions",
        "flows",
        "projects",
        "test_suite", // should be test_suites
    ];
}

pub fn load_complete_entities() -> HashMap<&'static str, Vec<String>> {
    TABLES_NAMES
        .iter()
        .map(|table_name| {
            let path = format!("{}/.config/qapi/{}.txt", HOME_DIR.display(), table_name);
            let f = File::open(path);
            if f.is_err() {
                return (*table_name, vec![]);
            }
            let mut f = f.unwrap();
            let mut contents: String = String::new();
            f.read_to_string(&mut contents).unwrap();
            let c_as_vec = contents.split('\n').map(|s| s.to_string()).collect();
            (*table_name, c_as_vec)
        })
        .collect()
}

pub async fn get_entities(pool: &SqlitePool) -> HashMap<String, String> {
    let mut r = HashMap::new();
    for table_name in TABLES_NAMES.iter() {
        let values = sqlx::query(&format!("SELECT name FROM {}", table_name))
            .map(|row: SqliteRow| row.get::<String, _>("name"))
            .fetch_all(pool)
            .await
            .unwrap_or(vec![])
            .join("\n");

        r.insert(table_name.to_string(), values);
    }
    r
}

pub fn write_complete_files(values: &HashMap<String, String>) {
    values.iter().for_each(|(name, value)| {
        let path = format!("{}/.config/qapi/{}.txt", HOME_DIR.display(), name);
        File::create(path)
            .unwrap()
            .write_all(value.as_bytes())
            .unwrap();
    });
}

pub fn get_and_export_complete_script() -> std::io::Result<ExitStatus> {
    let r = std::process::Command::new(env::current_exe().unwrap().to_str().unwrap())
        .arg("print-complete-script")
        .arg("zsh")
        .output()?;

    let autocomplete_file = format!("{}/.oh-my-zsh/completions/_apicrab", HOME_DIR.display());
    File::create(autocomplete_file)?.write_all(&r.stdout)?;

    let r = std::process::Command::new("/bin/zsh")
        .arg("-c")
        .arg("exec zsh")
        .status();
    match &r {
        Ok(status) => println!("Success {:?}", status),
        Err(e) => println!("Error... Sorry! {}", e),
    }
    r
}

pub async fn complete_update(pool: &SqlitePool) -> anyhow::Result<()> {
    let entities = get_entities(pool).await;
    write_complete_files(&entities);
    get_and_export_complete_script()?;
    Ok(())
}
