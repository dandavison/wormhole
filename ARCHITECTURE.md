# Wormhole Architecture

Wormhole is a project/task management system that integrates terminal (tmux), editor (Cursor/VSCode), and external services (GitHub, JIRA) into a unified workflow.

## Data Model

### StoreKey

The fundamental identifier for projects and tasks.

[src/project.rs (`StoreKey`)](https://github.com/dandavison/wormhole/blob/main/src/project.rs#L10-L46)
```rust
pub struct StoreKey {
    pub repo: String,
    pub branch: Option<String>,
}
```

- Projects: `StoreKey { repo: "myrepo", branch: None }` → displays as `"myrepo"`
- Tasks: `StoreKey { repo: "myrepo", branch: Some("feature") }` → displays as `"myrepo:feature"`

### Project

The core entity representing a git repository or task (worktree).

[src/project.rs (`Project`)](https://github.com/dandavison/wormhole/blob/main/src/project.rs#L48-L107)
```rust
pub struct Project {
    pub repo_name: String,
    pub repo_path: PathBuf,
    pub kv: HashMap<String, String>,
    pub last_application: Option<Application>,
    pub branch: Option<String>,
    pub github_pr: Option<u64>,
    pub github_repo: Option<String>,
}
```

Key methods:
- `is_task()` — returns true if `branch.is_some()`
- `store_key()` — returns the `StoreKey` for this project
- `worktree_path()` — for tasks, returns the worktree directory
- `is_open()` — checks if terminal window exists

### Projects Store

The in-memory store managing all projects with a navigation ring.

[src/projects.rs (`Store`)](https://github.com/dandavison/wormhole/blob/main/src/projects.rs#L25-L28)
```rust
struct Store {
    all: HashMap<StoreKey, Project>,
    ring: VecDeque<StoreKey>,
}
```

Ring semantics:
- Index 0: current project
- Index 1: previous project (what you just came from)
- Back: next project (oldest in ring)
- New projects inserted at index 1, then rotated to front

[src/projects.rs (`Projects`)](https://github.com/dandavison/wormhole/blob/main/src/projects.rs#L51-L78)
```rust
impl<'a> Projects<'a> {
    pub fn current(&self) -> Option<Project> {
        self.0.ring.front().and_then(|k| self.0.all.get(k)).cloned()
    }

    pub fn previous(&self) -> Option<Project> {
        self.0.ring.get(1).and_then(|k| self.0.all.get(k)).cloned()
    }

    pub fn next(&self) -> Option<Project> {
        self.0.ring.back().and_then(|k| self.0.all.get(k)).cloned()
    }
}
```

### Tasks (Worktrees)

Tasks are git worktrees managed by wormhole, stored at `.git/wormhole/worktrees/{branch}`.

[src/task.rs (`create_task`)](https://github.com/dandavison/wormhole/blob/main/src/task.rs#L34-L68)
```rust
pub fn create_task(repo: &str, branch: &str) -> Result<Project, String> {
    let repo_path = config::resolve_project_name(repo)?;
    let worktree_base = git::worktree_base_path(&repo_path);
    let worktree_path = worktree_base.join(branch);
    git::create_worktree(&repo_path, branch, &worktree_path)?;
    // ...creates .task/plan.md, .gitattributes
}
```

### KV Store

Per-project key-value storage, persisted as JSON files.

[src/kv.rs (`kv_file_for_key`)](https://github.com/dandavison/wormhole/blob/main/src/kv.rs#L96-L107)
```rust
fn kv_file_for_key(key: &StoreKey, repo_path: &Path) -> PathBuf {
    let filename = key.to_string().replace(':', "_");
    git::git_common_dir(repo_path)
        .join("wormhole/kv")
        .join(format!("{}.json", filename))
}
```

Storage location: `{git_common_dir}/wormhole/kv/{store_key}.json`


---

## Data Flow

### Startup

```
main.rs
  → projects::load()
    → config::available_projects()  // discover from WORMHOLE_PATH
    → git::list_worktrees()         // find tasks
    → kv::load_kv_data()            // load persisted KV
  → wormhole::serve_http()          // start HTTP server
```

### Project Switch

```
HTTP /project/switch/{name}
  → resolve project or create task
  → ProjectPath::open()
    → editor::open_workspace()
    → terminal::open()
    → hammerspoon::launch_or_focus()
  → projects.apply(Mutation::Insert)  // add to ring
```

### Task Creation

```
HTTP /project/create/{branch}?home-project={repo}
  → task::create_task()
    → git::create_worktree()
    → create .task/plan.md
    → projects.add()
  → task::open_task()
```

### Ring Navigation

```
HTTP /project/previous
  → projects.previous()           // get index 1
  → projects.apply(RotateLeft)    // move to front
  → ProjectPath::open()
```

### KV Operations

```
HTTP PUT /kv/{project}/{key}
  → kv::set_value()
    → projects.get_mut()
    → project.kv.insert()
    → kv::save_kv_data()          // persist to JSON
```

---

## HTTP API

All endpoints served from `http://127.0.0.1:7117` (configurable via `WORMHOLE_PORT`).

### Project Endpoints

#### GET /project/list

Returns current and available projects.

[src/endpoints.rs (`list_projects`)](https://github.com/dandavison/wormhole/blob/main/src/endpoints.rs#L8-L98)
```rust
pub fn list_projects(sprint_only: bool) -> Response<Body> {
    let open_projects = projects::lock().open();
    // ...filters if sprint_only, adds JIRA/PR info
}
```

Query parameters:
- `?sprint=true` — filter to tasks in current sprint, include JIRA status and PR info

Response:
```json
{
  "current": [{"name": "repo", "branch": "feature", "path": "/path", "kv": {...}}],
  "available": ["repo1", "repo2"]
}
```

#### GET /project/neighbors

Returns the ring for neighbor overlay navigation.

[src/wormhole.rs (`/project/neighbors`)](https://github.com/dandavison/wormhole/blob/main/src/wormhole.rs#L62-L76)
```rust
let ring: Vec<serde_json::Value> = projects
    .all()
    .iter()
    .map(|p| {
        let mut obj = serde_json::json!({ "name": p.repo_name });
        if let Some(branch) = &p.branch {
            obj["branch"] = serde_json::json!(branch);
        }
        obj
    })
    .collect();
```

#### GET /project/previous, /project/next

Navigate the ring. Returns empty body, side effect is switching project.

[src/wormhole.rs (`/project/previous`)](https://github.com/dandavison/wormhole/blob/main/src/wormhole.rs#L96-L110)
```rust
let p = {
    let mut projects = projects::lock();
    let pp = projects.previous().map(|p| p.as_project_path());
    if let Some(ref pp) = pp {
        projects.apply(Mutation::RotateLeft, &pp.project.store_key());
    }
    pp
};
if let Some(project_path) = p {
    thread::spawn(move || project_path.open_with_options(Mutation::None, land_in, skip_editor));
}
```

Query parameters:
- `?land-in=terminal|editor` — which app to focus
- `?skip-editor=true` — don't open/focus editor

#### GET /project/switch/{name_or_path}

Switch to a project or create a task.

[src/wormhole.rs (`/project/switch`)](https://github.com/dandavison/wormhole/blob/main/src/wormhole.rs#L287-L345)
```rust
if let (Some(repo), Some(branch)) = (repo.as_ref(), branch.as_ref()) {
    return crate::task::open_task(repo, branch, land_in, skip_editor, focus_terminal);
}
if let Some((repo, branch)) = name_or_path.split_once(':') {
    // Handle store_key format "repo:branch"
}
```

Query parameters:
- `?home-project={repo}` — for task creation
- `?branch={branch}` — for task creation
- `?land-in=terminal|editor`
- `?skip-editor=true`
- `?sync=true` — wait for completion

#### POST /project/describe

Describe a URL (GitHub or JIRA) and find associated wormhole task.

[src/describe.rs (`describe`)](https://github.com/dandavison/wormhole/blob/main/src/describe.rs#L43-L53)
```rust
pub fn describe(url: &str) -> DescribeResponse {
    if let Some(gh) = parse_github_url(url) {
        return describe_github(gh);
    }
    if let Some(jira) = parse_jira_url(url) {
        return describe_jira(&jira.key);
    }
    DescribeResponse::default()
}
```

Used by Chrome extension to show wormhole buttons on GitHub PRs and JIRA issues.

#### GET /project/show/{name}

Get detailed status for a project/task.

[src/status.rs (`get_status`)](https://github.com/dandavison/wormhole/blob/main/src/status.rs#L43-L88)
```rust
pub fn get_status(project: &Project) -> TaskStatus {
    // Fetches JIRA info, PR status, plan existence concurrently
    TaskStatus {
        name, path, branch, jira, pr, plan_exists, plan_url, aux_repos,
    }
}
```

Query parameters:
- `?format=json` — return JSON instead of terminal-formatted text

#### POST /project/pin

Save current (project, application) state for restoration.

[src/endpoints.rs (`pin_project`)](https://github.com/dandavison/wormhole/blob/main/src/endpoints.rs#L186-L198)
```rust
pub fn pin_project() -> Response<Body> {
    let mut projects = projects::lock();
    if let Some(current) = projects.current() {
        let app = hammerspoon::current_application();
        projects.set_last_application(&current.store_key(), app);
    }
}
```

#### POST /project/close/{name}, /project/remove/{name}

Close windows or remove project from store.

[src/endpoints.rs (`close_project`)](https://github.com/dandavison/wormhole/blob/main/src/endpoints.rs#L140-L152)
```rust
pub fn close_project(name: &str) {
    let key = StoreKey::parse(name);
    let mut projects = projects::lock();
    if let Some(project) = projects.by_key(&key) {
        config::TERMINAL.close(&project);
        editor::close(&project);
    }
}
```

#### POST /project/refresh

Refresh all in-memory data from external sources.

[src/endpoints.rs (`refresh_all`)](https://github.com/dandavison/wormhole/blob/main/src/endpoints.rs#L154-L184)
```rust
pub fn refresh_all() {
    projects::refresh_tasks();
    crate::kv::load_kv_data(&mut projects);
    // Refresh GitHub info for all projects concurrently
    keys.par_iter().for_each(|key| {
        crate::github::refresh_github_info(project);
    });
}
```

### KV Endpoints

#### GET /kv, GET /kv/{project}, GET /kv/{project}/{key}

[src/kv.rs (`get_value`)](https://github.com/dandavison/wormhole/blob/main/src/kv.rs#L11-L29)
```rust
pub fn get_value(store_key: &StoreKey, key: &str) -> Option<String> {
    let projects = projects::lock();
    projects.by_key(store_key).and_then(|p| p.kv.get(key).cloned())
}
```

#### PUT /kv/{project}/{key}

[src/kv.rs (`set_value`)](https://github.com/dandavison/wormhole/blob/main/src/kv.rs#L31-L47)
```rust
pub fn set_value(store_key: &StoreKey, key: &str, value: &str) {
    let mut projects = projects::lock();
    if let Some(project) = projects.get_mut(store_key) {
        project.kv.insert(key.to_string(), value.to_string());
        save_kv_data(project);
    }
}
```

### Other Endpoints

#### GET /api/sprint

Returns sprint status for dashboard.

[src/status.rs (`get_sprint_status`)](https://github.com/dandavison/wormhole/blob/main/src/status.rs#L106-L125)
```rust
pub fn get_sprint_status() -> Vec<SprintShowItem> {
    let issues = jira::get_sprint_issues()?;
    issues.into_iter().map(|issue| {
        match projects.find_by_jira_key(&issue.key) {
            Some(project) => SprintShowItem::Task(get_status(project)),
            None => SprintShowItem::Issue(issue),
        }
    }).collect()
}
```

#### GET /dashboard

Returns HTML dashboard page.

[src/endpoints.rs (`dashboard`)](https://github.com/dandavison/wormhole/blob/main/src/endpoints.rs#L209-L225)

#### GET /shell?pwd={path}

Returns shell environment variables for a project.

[src/terminal.rs (`shell_env_code`)](https://github.com/dandavison/wormhole/blob/main/src/terminal.rs#L97-L107)
```rust
pub fn shell_env_code(project: &Project) -> String {
    let jira_url = jira_url_for_name(project);
    format!(
        "export WORMHOLE_PROJECT={}\nexport WORMHOLE_JIRA_URL={}",
        project.store_key(), jira_url.unwrap_or_default()
    )
}
```

---

## CLI Commands

### Server Commands

#### `wormhole server start`

Starts the daemon in a tmux session.

[src/cli.rs (`Server::Start`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L444-L456)
```rust
ServerCommand::Start => {
    daemon::daemon().create_or_attach(None)?;
    let pid = std::process::id();
    daemon::daemon().send_keys(format!("target/debug/wormhole server start-foreground {}", pid))?;
}
```

### Project Commands

#### `wormhole project switch <name>`

Switch to a project or create a task.

[src/cli.rs (`Project::Switch`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L473-L487)
```rust
ProjectCommand::Switch { name_or_path, name, land_in, home_project, branch } => {
    let branch = prompt_for_branch_if_needed(&client, &name_or_path, &home_project, branch)?;
    let query = build_switch_query(&land_in, &name, &home_project, &branch);
    client.get(&format!("/project/switch/{}{}", name_or_path, query))?;
}
```

#### `wormhole project list [-o json] [--available]`

List current or available projects.

[src/cli.rs (`Project::List`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L488-L508)
```rust
ProjectCommand::List { output, available } => {
    let response = client.get("/project/list")?;
    // render_project_item() for each project
}
```

#### `wormhole project show [name] [-o json]`

Show detailed status for a project/task.

[src/cli.rs (`Project::Show`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L544-L558)
```rust
ProjectCommand::Show { name, output } => {
    let path = match name {
        Some(n) => format!("/project/show/{}", n),
        None => format!("/project/show/{}", cwd),
    };
    let query = if output == "json" { "?format=json" } else { "" };
    let response = client.get(&format!("{}{}", path, query))?;
}
```

### KV Commands

#### `wormhole kv get <project> <key>`

[src/cli.rs (`Kv::Get`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L569-L589)
```rust
KvCommand::Get { project, key, output } => {
    let response = client.get(&format!("/kv/{}/{}", project, key));
    let kv = KvValue { project, key, value: response.ok() };
    // render as JSON or text
}
```

#### `wormhole kv set <project> <key> <value>`

[src/cli.rs (`Kv::Set`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L590-L597)
```rust
KvCommand::Set { project, key, value } => {
    client.put(&format!("/kv/{}/{}", project, key), &value)?;
}
```

### JIRA Commands

#### `wormhole jira sprint [-o json]`

List sprint issues (filtered to tasks with wormhole projects).

[src/cli.rs (`sprint_list`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L1070-L1082)
```rust
fn sprint_list(client: &Client, output: &str) -> Result<(), String> {
    let response = client.get("/project/list?sprint=true")?;
    // render_project_item(item, true) for sprint view
}
```

#### `wormhole jira sprint show [-o json]`

Detailed status for each sprint issue.

[src/cli.rs (`sprint_show`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L1156-L1208)

#### `wormhole jira sprint create`

Interactive task creation from sprint issues.

[src/cli.rs (`sprint_create`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L825-L1058)
```rust
fn sprint_create(client: &Client, overrides: Vec<String>, output: &str) -> Result<(), String> {
    let issues = jira::get_sprint_issues()?;
    // Interactive selection with rustyline
    // Creates task via /project/switch with home-project and branch
}
```

### Other Commands

#### `wormhole refresh`

Refresh in-memory data from external sources.

[src/cli.rs (`Refresh`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L678-L682)
```rust
Command::Refresh => {
    client.post("/project/refresh")?;
    println!("Refreshed");
}
```

#### `wormhole doctor persisted-data [-o json]`

Report on persisted wormhole data (worktrees, KV files).

[src/cli.rs (`doctor_persisted_data`)](https://github.com/dandavison/wormhole/blob/main/src/cli.rs#L739-L823)
```rust
fn doctor_persisted_data(output: &str) -> Result<(), String> {
    let available = config::available_projects();
    // For each project: list worktrees and KV files from disk
}
```

---

## Key Modules

### config.rs — Configuration

[src/config.rs](https://github.com/dandavison/wormhole/blob/main/src/config.rs)

- `editor()` — returns configured editor (Cursor, VSCode, etc.)
- `wormhole_port()` — HTTP port (default 7117, env `WORMHOLE_PORT`)
- `available_projects()` — discovers projects from `WORMHOLE_PATH`
- `resolve_project_name()` — resolves name to path

### git.rs — Git Operations

[src/git.rs](https://github.com/dandavison/wormhole/blob/main/src/git.rs)

- `git_common_dir()` — finds `.git` or submodule git dir
- `list_worktrees()` — parses `git worktree list`
- `create_worktree()` — creates new worktree
- `worktree_base_path()` — `.git/wormhole/worktrees`

### github.rs — GitHub Integration

[src/github.rs](https://github.com/dandavison/wormhole/blob/main/src/github.rs)

- `get_pr_status()` — fetches PR via `gh pr view`
- `refresh_github_info()` — updates cached PR info

### jira.rs — JIRA Integration

[src/jira.rs](https://github.com/dandavison/wormhole/blob/main/src/jira.rs)

- `get_issue()` — fetches issue by key
- `get_sprint_issues()` — fetches current sprint via JQL

### terminal.rs — Terminal Integration

[src/terminal.rs](https://github.com/dandavison/wormhole/blob/main/src/terminal.rs)

Supports Wezterm (native) and Alacritty+tmux.

- `open()` — creates terminal window/tmux pane
- `close()` — closes window
- `focus()` — focuses terminal
- `exists()` — checks if window exists

### editor.rs — Editor Integration

[src/editor.rs](https://github.com/dandavison/wormhole/blob/main/src/editor.rs)

Supports Cursor, VSCode, VSCodeInsiders, Emacs, IntelliJ, PyCharm.

- `open_workspace()` — opens project directory
- `open_path()` — opens file at line
- `close()` — closes editor window
- `focus()` — focuses editor

### hammerspoon.rs — macOS Integration

[src/hammerspoon.rs](https://github.com/dandavison/wormhole/blob/main/src/hammerspoon.rs)

- `current_application()` — detects focused app
- `launch_or_focus()` — focuses application
- `close_window()` — closes windows by pattern
