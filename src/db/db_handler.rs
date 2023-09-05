use crate::db::dao::{Context, History, LightAction, Project};
use colored::Colorize;
use sqlx::{sqlite::SqlitePool, Executor, Row};

#[derive(sqlx::FromRow)]
pub struct Flow {
    pub name: String,
    pub actions: String,
    // list of action names
    pub bodies: Option<String>,
    pub path_params: Option<String>,
    pub extracted_path: Option<String>,
}

static INIT_TABLES: &str = r#"
BEGIN TRANSACTION;
CREATE TABLE projects (
    name TEXT PRIMARY KEY,
    test_url TEXT,
    prod_url TEXT,
    conf TEXT
);
CREATE TABLE actions (
    name TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    verb TEXT NOT NULL,
    headers TEXT,
    static_body TEXT,
    body_example TEXT,
    response_example TEXT,
    project_name TEXT NOT NULL,
    FOREIGN KEY(project_name) REFERENCES projects(name)
    CONSTRAINT unique_action UNIQUE (name, project_name)
);
CREATE TABLE flows (
    name PRIMARY KEY,
    actions TEXT NOT NULL,
    bodies TEXT,
    path_params TEXT,
    extracted_path TEXT
);
CREATE TABLE history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action_name TEXT NOT NULL,
    url TEXT NOT NULL,
    body TEXT,
    headers TEXT,
    response TEXT,
    status_code INTEGER NOT NULL,
    duration REAL NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(action_name) REFERENCES actions(name)
);

CREATE TABLE context (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    value TEXT
);
COMMIT;
"#;

/// Error messages
static MISSING_HOME: &str = "Missing HOME env variable";
static PROJECT_NOT_FOUND: &str =
    "Project not found. Did you forget to create it running `qapi create-project <project_name>`?";
static CONNECTION_ERROR: &str = "Connection to database failed";

pub struct DBHandler {
    conn: Option<SqlitePool>,
}

impl DBHandler {
    pub fn new() -> Self {
        Self { conn: None }
    }

    fn get_conn(&self) -> &SqlitePool {
        self.conn.as_ref().expect(CONNECTION_ERROR.as_ref())
    }

    /// Create database if needed at the startup of the application
    pub async fn init_db(&mut self) -> anyhow::Result<()> {
        let home_dir = std::env::home_dir().ok_or(anyhow::anyhow!(MISSING_HOME.red()))?;
        let path_as_str = format!("{}/.config/qapi/qapi.sqlite", home_dir.display());
        let path = std::path::Path::new(path_as_str.as_str());

        let sqlite_uri = format!("file:{}", path_as_str);

        if path.exists() {
            self.conn = SqlitePool::connect(sqlite_uri.as_str()).await.ok();
            return Ok(());
        }
        let parent = path
            .parent()
            .ok_or(anyhow::anyhow!("Missing parent".red()))?;
        std::fs::create_dir_all(parent)?;
        std::fs::File::create(path_as_str.clone())?;

        self.conn = SqlitePool::connect(sqlite_uri.as_str()).await.ok();

        let conn = self.get_conn();

        conn.execute(INIT_TABLES).await?;
        Ok(())
    }

    /// Return the project id if it exists for a given project name
    pub async fn get_project(&self, project_name: &str) -> anyhow::Result<Project> {
        let project_opt = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE name = ?1")
            .bind(project_name)
            .fetch_optional(self.get_conn())
            .await?;

        project_opt.ok_or(anyhow::anyhow!(PROJECT_NOT_FOUND.red()))
    }

    /// create a new project with the given name
    pub async fn upsert_project(&self, project: &Project) -> anyhow::Result<i64> {
        let r = sqlx::query(
            r#"
            INSERT INTO projects (name, test_url, prod_url, conf)
            VALUES (?1, ?2, ?3, ?4) ON CONFLICT (name)
            DO UPDATE SET test_url = ?2, prod_url = ?3, conf= ?4;
            "#,
        )
        .bind(&project.name)
        .bind(&project.test_url)
        .bind(&project.prod_url)
        .bind(&project.conf)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        match r {
            0 => println!("{}", "Project updated".blue()),
            _ => println!("{}", "Project successfully created !".yellow()),
        }
        Ok(r)
    }

    pub async fn list_projects(&self) -> anyhow::Result<()> {
        println!("{}", "Projects:".blue());
        sqlx::query("SELECT name FROM projects")
            .fetch_all(self.get_conn())
            .await?
            .into_iter()
            .for_each(|project| println!("{}", project.get::<String, _>(0)));
        Ok(())
    }

    /// list information about a project i.e. its configured urls and associated actions
    pub async fn list_project_data(&self, project_name: &str) -> anyhow::Result<()> {
        let project = self.get_project(project_name).await?;

        println!("{}", "Configured actions:".yellow());
        sqlx::query_as::<_, LightAction>(
            r#"
            SELECT *
            FROM actions
            WHERE project_name = ?1
            "#,
        )
        .bind(project.name)
        .fetch_all(self.get_conn())
        .await?
        .iter()
        .for_each(|light_action| println!("{}", light_action.to_string()));

        Ok(())
    }

    pub async fn upsert_action(&self, light_action: &LightAction) -> anyhow::Result<()> {
        let r = sqlx::query(
            r#"
            INSERT INTO actions (name, url, verb, static_body, headers, body_example, response_example, project_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT (name, project_name)
            DO UPDATE SET url = ?2, verb = ?3, static_body = ?4, headers = ?5, body_example = ?6, response_example = ?7;
            "#,
        )
            .bind(&light_action.name)
            .bind(&light_action.url)
            .bind(&light_action.verb)
            .bind(&light_action.static_body)
            .bind(&light_action.headers)
            .bind(&light_action.body_example)
            .bind(&light_action.response_example)
            .bind(&light_action.project_name)
            .execute(self.get_conn())
            .await?
            .last_insert_rowid();

        match r {
            0 => println!("{}", "Action updated".blue()),
            _ => println!("{}", "Action successfully added".yellow()),
        }
        Ok(())
    }

    pub async fn get_action(&self, action_name: &str) -> anyhow::Result<LightAction> {
        let action = sqlx::query_as::<_, LightAction>(
            r#"
            SELECT *
            FROM actions a
            WHERE a.name = ?1
            "#,
        )
        .bind(action_name)
        .fetch_one(self.get_conn())
        .await
        .expect(format!("Action {} not found", action_name).as_str());

        Ok(action)
    }

    pub async fn rm_action(&self, action_name: &str) -> anyhow::Result<u64> {
        let r = sqlx::query("DELETE FROM actions WHERE name = ?1;")
            .bind(action_name)
            .execute(self.get_conn())
            .await?
            .rows_affected();

        match r {
            0 => println!("{}", "Action not found".red()),
            _ => println!("{}", "Action successfully deleted".blue()),
        }
        Ok(r)
    }

    pub async fn get_flow(&self, flow_name: &str) -> anyhow::Result<Flow> {
        let flow = sqlx::query_as::<_, Flow>(
            r#"
            SELECT name, actions, bodies, path_params, extracted_path
            FROM flows
            WHERE name = ?1
            "#,
        )
        .bind(flow_name)
        .fetch_one(self.get_conn())
        .await?;

        Ok(flow)
    }

    pub async fn upsert_flow(
        &self,
        flow_name: &str,
        actions: &Vec<String>,
        bodies: &Option<Vec<String>>,
        path_params: &Option<Vec<String>>,
        extracted_path: &Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let r = sqlx::query(
            r#"
            INSERT INTO flows (name, actions, bodies, path_params, extracted_path)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT (name)
            DO UPDATE SET actions = ?2, bodies = ?3, path_params = ?4, extracted_path = ?5;
            "#,
        )
        .bind(flow_name)
        .bind(serde_json::to_string(&actions)?)
        .bind(serde_json::to_string(&bodies)?)
        .bind(serde_json::to_string(&path_params)?)
        .bind(serde_json::to_string(&extracted_path)?)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        match r {
            0 => println!("{}", "Flow updated".blue()),
            _ => println!("{}", "Flow successfully added".yellow()),
        }
        Ok(())
    }

    pub async fn get_conf(&self) -> anyhow::Result<Context> {
        let conf = sqlx::query_as::<_, Context>(
            r#"
            SELECT value
            FROM context
            "#,
        )
        .fetch_one(self.get_conn())
        .await?;
        Ok(conf)
    }

    pub async fn insert_conf(&self, context: &Context) -> anyhow::Result<i64> {
        let r = sqlx::query(
            r#"
            BEGIN;
            DELETE FROM context;
            INSERT INTO context (value) VALUES (?1);
            COMMIT;
            "#,
        )
        .bind(&context.value)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        Ok(r)
    }

    pub async fn insert_history(&self, history: &History) -> anyhow::Result<i64> {
        let r = sqlx::query(
            r#"
            INSERT INTO history (action_name, url, body, headers, response, status_code, duration)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
            "#,
        )
        .bind(&history.action_name)
        .bind(&history.url)
        .bind(&history.body)
        .bind(&history.headers)
        .bind(&history.response)
        .bind(&history.status_code)
        .bind(&history.duration)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        Ok(r)
    }

    pub async fn get_history(&self) -> anyhow::Result<Vec<History>> {
        let history = sqlx::query_as::<_, History>(
            r#"
            SELECT *
            FROM history
            ORDER BY timestamp DESC
            limit 20;
            "#,
        )
        .fetch_all(self.get_conn())
        .await?;

        Ok(history)
    }
}
