use sqlx::{sqlite::SqlitePool, Executor, Row};

#[derive(sqlx::FromRow)]
pub struct Action {
    pub project_name: String,
    pub name: String,
    pub url: String,
    pub verb: String,
    pub env: String,
    pub main_url: String,
    pub api_key: String,
    pub body_example: Option<String>,
    pub response_example: Option<String>,
}

impl Action {
    pub fn full_url(&self) -> String {
        format!("{}/{}", self.main_url, self.url)
    }
}

#[derive(sqlx::FromRow)]
pub struct LightAction {
    pub name: String,
    pub url: String,
    pub verb: String,
    pub body_example: String,
    pub response_example: String,
}

#[derive(sqlx::FromRow)]
pub struct Url {
    pub url: String,
    pub env: String,
    pub conf: String,
}

static INIT_TABLES: &str = r#"
BEGIN TRANSACTION;
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL
);
CREATE TABLE urls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT UNIQUE NOT NULL,
    api_key TEXT,
    env TEXT DEFAULT 'test',
    conf TEXT DEFAULT '{}',
    project_id INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);
CREATE TABLE actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    url TEXT NOT NULL UNIQUE,
    verb TEXT NOT NULL,
    url_id INTEGER NOT NULL,
    body_example TEXT,
    response_example TEXT,
    FOREIGN KEY(url_id) REFERENCES urls(id)
);
COMMIT;
"#;

/// Error messages
static MISSING_HOME: &str = "Missing HOME env variable";
static PROJECT_NOT_FOUND: &str =
    "Project not found. Did you forget to create it running `qapi create-project <project_name>`?";
static URL_NOT_FOUND: &str = r#"
    Url not found.
    Did you forget to add it running `qapi add-url <project_name> <url> <api_key> <env>`?
"#;
static CONNECTION_ERROR: &str = "Connection to database failed";

pub struct DBHandler {
    conn: Option<SqlitePool>,
}

impl DBHandler {
    pub fn new() -> Self {
        Self { conn: None }
    }

    fn get_conn(&self) -> &SqlitePool {
        self.conn.as_ref().expect(CONNECTION_ERROR)
    }

    /// Create database if needed at the startup of the application
    pub async fn init_db(&mut self) -> anyhow::Result<()> {
        let home_dir = std::env::home_dir().ok_or(anyhow::anyhow!(MISSING_HOME))?;
        let path_as_str = format!("{}/.config/qapi/qapi.sqlite", home_dir.display());
        let path = std::path::Path::new(path_as_str.as_str());

        let sqlite_uri = format!("file:{}", path_as_str);

        if path.exists() {
            self.conn = SqlitePool::connect(sqlite_uri.as_str()).await.ok();
            return Ok(());
        }
        println!("trying to create file at {}", path_as_str);
        let parent = path.parent().ok_or(anyhow::anyhow!("Missing parent"))?;
        std::fs::create_dir_all(parent)?;
        std::fs::File::create(path_as_str.clone())?;

        self.conn = SqlitePool::connect(sqlite_uri.as_str()).await.ok();

        if self.conn.is_none() {
            println!("{}", CONNECTION_ERROR);
        }

        let conn = self.get_conn();

        conn.execute(INIT_TABLES).await?;
        Ok(())
    }

    /// Return the project id if it exists for a given project name
    pub async fn get_project_id(&self, project_name: &str) -> anyhow::Result<Option<i64>> {
        let row = sqlx::query("SELECT id FROM projects WHERE name = ?1")
            .bind(project_name)
            .fetch_optional(self.get_conn())
            .await?;

        match row {
            Some(row) => Ok(Some(row.get(0))),
            None => Ok(None),
        }
    }

    /// create a new project with the gievn name
    pub async fn create_project(&self, name: &str) -> anyhow::Result<u64> {
        let r =
            sqlx::query("INSERT INTO projects (name) VALUES (?1) ON CONFLICT (name) DO NOTHING")
                .bind(name)
                .execute(self.get_conn())
                .await?
                .rows_affected();

        match r {
            0 => println!("Project already exists"),
            _ => println!("Project successfully created"),
        }
        Ok(r)
    }

    /// Add a new url to a project
    pub async fn add_url_to_project(
        &self,
        project_name: &str,
        url: &str,
        conf: &Vec<String>,
        env: &str,
    ) -> anyhow::Result<()> {
        let project_id = self
            .get_project_id(project_name)
            .await?
            .expect(PROJECT_NOT_FOUND);

        let conf_map = conf
            .into_iter()
            .map(|c| {
                let mut split = c.split(":");
                let key = split.next().unwrap(); //.expect("Incorrect configuration").to_string();
                let value = split.next().unwrap(); //.expect("Incorrect configuration").to_string();
                (key, value)
            })
            .collect::<std::collections::HashMap<_, _>>();

        let r = sqlx::query(
            r#"
            INSERT INTO urls (url, conf, env, project_id) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT (url)
            DO UPDATE SET conf = ?2, env = ?3, project_id = ?4;
            "#,
        )
        .bind(url)
        .bind(serde_json::to_string(&conf_map).unwrap())
        .bind(env)
        .bind(project_id)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        match r {
            0 => println!("Url successfully updated"),
            _ => println!("Url successfully added"),
        }
        Ok(())
    }

    pub async fn list_projects(&self) -> anyhow::Result<()> {
        println!("Projects:");
        sqlx::query("SELECT name FROM projects")
            .fetch_all(self.get_conn())
            .await?
            .into_iter()
            .for_each(|project| println!("{}", project.get::<String, _>(0)));
        Ok(())
    }

    /// list information about a project i.e. its configured urls and associated actions
    pub async fn list_project_url(&self, project_name: &str) -> anyhow::Result<()> {
        let project_id = self
            .get_project_id(project_name)
            .await?
            .expect(PROJECT_NOT_FOUND);

        println!("Project {} urls:", project_name);
        sqlx::query_as::<_, Url>("SELECT url, conf, env FROM urls WHERE project_id = ?1")
            .bind(project_id)
            .fetch_all(self.get_conn())
            .await?
            .into_iter()
            .for_each(|action| {
                println!(
                    "url: {}, conf: {}, env: {}",
                    action.url, action.conf, action.env
                );
            });
        println!("Configured actions:");
        sqlx::query_as::<_, LightAction>(
            r#"
            SELECT name, url, verb, body_example, response_example
            FROM actions
            WHERE url_id IN (SELECT id FROM urls WHERE project_id = ?1)
            "#,
        )
        .bind(project_id)
        .fetch_all(self.get_conn())
        .await?
        .into_iter()
        .for_each(|light_action| {
            println!(
                "name: {}, url: {}, verb: {}, body_example: {}, response_example: {}",
                light_action.name,
                light_action.url,
                light_action.verb,
                light_action.body_example,
                light_action.response_example
            );
        });

        Ok(())
    }

    pub async fn upsert_action(
        &self,
        project_name: &str,
        name: &str,
        url: &str,
        verb: &str,
        env: &str,
        body_example: &Option<String>,
        response_example: &Option<String>,
    ) -> anyhow::Result<()> {
        let project_id = self
            .get_project_id(project_name)
            .await?
            .expect(PROJECT_NOT_FOUND);

        let url_row = sqlx::query(
            r#"
            SELECT id FROM urls WHERE project_id = ?1 AND env = ?2;
            "#,
        )
        .bind(project_id)
        .bind(env)
        .fetch_one(self.get_conn())
        .await?;

        if url_row.is_empty() {
            println!("{}", URL_NOT_FOUND);
            return Ok(());
        }
        let url_id = url_row.get::<i64, _>(0);

        let r = sqlx::query(
            r#"
            INSERT INTO actions (name, url, verb, url_id, body_example, response_example)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT (name)
            DO UPDATE SET url = ?2, verb = ?3, url_id = ?4, body_example = ?5, response_example = ?6;
            "#,
        )
        .bind(name)
        .bind(url)
        .bind(verb)
        .bind(url_id)
        .bind(body_example)
        .bind(response_example)
        .execute(self.get_conn())
        .await?
        .last_insert_rowid();

        match r {
            0 => println!("Action updated"),
            _ => println!("Action successfully added"),
        }
        Ok(())
    }

    pub async fn get_action(&self, action_name: &str) -> anyhow::Result<Action> {
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT p.name as project_name,
                   a.name,
                   a.url,
                   a.verb,
                   u.env,
                   u.url as main_url,
                   u.api_key,
                   a.body_example,
                   a.response_example
            FROM actions a
            INNER JOIN urls u ON a.url_id = u.id
            INNER JOIN projects p ON u.project_id = p.id
            WHERE a.name = ?1
            "#,
        )
        .bind(action_name)
        .fetch_one(self.get_conn())
        .await?;

        Ok(action)
    }

    pub async fn delete_action(&self, action_name: &str) -> anyhow::Result<u64> {
        let r = sqlx::query("DELETE FROM actions WHERE name = ?1;")
            .bind(action_name)
            .execute(self.get_conn())
            .await?
            .rows_affected();

        match r {
            0 => println!("Action not found"),
            _ => println!("Action successfully deleted"),
        }
        Ok(r)
    }
}
