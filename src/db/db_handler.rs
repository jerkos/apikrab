use crate::db::dto::{Action, Context, History, Project, TestSuite, TestSuiteInstance};
use crate::HOME_DIR;
use colored::Colorize;
use sqlx::{sqlite::SqlitePool, Executor};

static INIT_TABLES: &str = r#"
BEGIN TRANSACTION;

CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    main_url TEXT NOT NULL,
    conf TEXT DEFAULT '{}',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NULLABLE,
    CONSTRAINT unique_project_name UNIQUE (name)
);

CREATE TABLE actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NULLABLE,
    run_action_args TEXT NULLABLE,
    body_example TEXT NULLABLE,
    response_example TEXT NULLABLE,
    project_name TEXT NULLABLE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NULLABLE,
    FOREIGN KEY(project_name) REFERENCES projects(name)
    CONSTRAINT unique_action UNIQUE (name, project_name)
);

CREATE TABLE history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action_name TEXT,
    url TEXT NOT NULL,
    body TEXT,
    headers TEXT,
    response TEXT,
    status_code INTEGER NOT NULL,
    duration REAL NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(action_name) REFERENCES actions(name)
);

CREATE TABLE test_suite (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT unique_test_suite_name UNIQUE (name)
);

CREATE TABLE test_suite_steps(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    test_suite_name TEXT NOT NULL,
    run_action_args TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NULLABLE,
    FOREIGN KEY(test_suite_name) REFERENCES test_suite(name)
);

CREATE TABLE context (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    value TEXT
);
COMMIT;
"#;

/// Error messages
static PROJECT_NOT_FOUND: &str =
    "Project not found. Did you forget to create it running `apikrab project new <project_name>`?";
static CONNECTION_ERROR: &str = "Connection to database failed";

#[derive(Clone)]
pub struct DBHandler {
    pub conn: Option<SqlitePool>,
}

impl DBHandler {
    pub fn new() -> Self {
        Self { conn: None }
    }

    fn get_conn(&self) -> &SqlitePool {
        self.conn
            .as_ref()
            .unwrap_or_else(|| panic!("{}", CONNECTION_ERROR))
    }

    /// Create database if needed at the startup of the application
    pub async fn init_db(&mut self) -> anyhow::Result<()> {
        let path_as_str = format!("{}/.config/qapi/qapi.sqlite", HOME_DIR.display());

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
            INSERT INTO projects (id, name, main_url, conf, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (name)
            DO UPDATE SET main_url = ?3, conf = ?4, updated_at = CURRENT_TIMESTAMP;
            "#,
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.main_url)
        .bind(&project.conf)
        .bind(&project.created_at)
        .bind(&project.updated_at)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();
        Ok(r)
    }

    pub async fn get_projects(&self) -> anyhow::Result<Vec<Project>> {
        let r = sqlx::query_as::<_, Project>("SELECT * FROM projects")
            .fetch_all(self.get_conn())
            .await?;
        Ok(r)
    }

    pub async fn upsert_action(&self, action: &Action) -> anyhow::Result<()> {
        let _ = sqlx::query(
            r#"
            INSERT INTO actions (
                id,
                name,
                run_action_args,
                body_example,
                response_example,
                project_name,
                created_at,
                updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT (name, project_name)
            DO UPDATE SET
                run_action_args = ?3,
                body_example = ?4,
                response_example = ?5,
                updated_at = CURRENT_TIMESTAMP;
            "#,
        )
        .bind(&action.id)
        .bind(&action.name)
        .bind(&action.run_action_args)
        .bind(&action.body_example)
        .bind(&action.response_example)
        .bind(&action.project_name)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();
        Ok(())
    }

    pub async fn get_actions(&self, project_name: &str) -> anyhow::Result<Vec<Action>> {
        let actions = sqlx::query_as::<_, Action>(
            r#"
            SELECT *
            FROM actions a
            WHERE a.project_name = ?1
            "#,
        )
        .bind(project_name)
        .fetch_all(self.get_conn())
        .await?;

        Ok(actions)
    }

    pub async fn get_action(&self, action_name: &str) -> anyhow::Result<Action> {
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT *
            FROM actions a
            WHERE a.name = ?1
            "#,
        )
        .bind(action_name)
        .fetch_one(self.get_conn())
        .await
        .unwrap_or_else(|_| panic!("Action {} not found", action_name));

        Ok(action)
    }

    pub async fn rm_action(&self, action_name: &str) -> anyhow::Result<u64> {
        let r = sqlx::query("DELETE FROM actions WHERE name = ?1;")
            .bind(action_name)
            .execute(self.get_conn())
            .await?
            .rows_affected();
        Ok(r)
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
            INSERT INTO history (id, action_name, url, body, headers, response, status_code, duration)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);
            "#,
        )
        .bind(&history.id)
        .bind(&history.action_name)
        .bind(&history.url)
        .bind(&history.body)
        .bind(&history.headers)
        .bind(&history.response)
        .bind(history.status_code)
        .bind(history.duration)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        Ok(r)
    }

    pub async fn get_history(&self, limit: Option<u16>) -> anyhow::Result<Vec<History>> {
        let _limit = limit.unwrap_or(20);
        let history = sqlx::query_as::<_, History>(
            format!(
                r#"
            SELECT *
            FROM history
            ORDER BY timestamp DESC
            limit {};
            "#,
                _limit
            )
            .as_str(),
        )
        .fetch_all(self.get_conn())
        .await?;

        Ok(history)
    }

    pub async fn upsert_test_suite(&self, test_suite_name: &str) -> anyhow::Result<()> {
        let _ = sqlx::query(
            r#"
            INSERT INTO test_suite (name)
            VALUES (?1)
            ON CONFLICT
            DO NOTHING;
            "#,
        )
        .bind(test_suite_name)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();
        Ok(())
    }

    pub async fn get_test_suite(&self, test_suite_name: &str) -> anyhow::Result<TestSuite> {
        let r = sqlx::query_as::<_, TestSuite>(
            r#"
            SELECT *
            FROM test_suite
            WHERE name = ?1
            "#,
        )
        .bind(test_suite_name)
        .fetch_one(self.get_conn())
        .await;
        match r {
            Ok(test_suite) => Ok(test_suite),
            Err(..) => anyhow::bail!("Flow not found"),
        }
    }

    pub async fn upsert_test_suite_instance(
        &self,
        test_suite_instance: &TestSuiteInstance,
    ) -> anyhow::Result<()> {
        let _ = sqlx::query(
            r#"
            INSERT INTO test_suite_steps (id, test_suite_name, run_action_args, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT (id)
            DO UPDATE SET run_action_args = ?3, updated_at = CURRENT_TIMESTAMP;
            "#,
        )
        .bind(&test_suite_instance.id)
        .bind(&test_suite_instance.test_suite_name)
        .bind(&test_suite_instance.run_action_args)
        .bind(&test_suite_instance.created_at)
        .bind(&test_suite_instance.updated_at)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();
        Ok(())
    }

    pub async fn get_test_suite_instance(
        &self,
        test_suite_name: &str,
    ) -> anyhow::Result<Vec<TestSuiteInstance>> {
        let r = sqlx::query_as::<_, TestSuiteInstance>(
            r#"
            SELECT *
            FROM test_suite_steps
            WHERE test_suite_name = ?1
            "#,
        )
        .bind(test_suite_name)
        .fetch_all(self.get_conn())
        .await?;

        Ok(r)
    }
}
