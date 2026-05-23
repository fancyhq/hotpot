use std::{
    ffi::OsStr,
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use nanoid::nanoid;
use serde::Serialize;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::{context, paths};

const FRAME_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Hotpot Visual Companion</title>
  <style>
    * { box-sizing: border-box; }
    :root {
      --bg: #f7f7f8;
      --panel: #ffffff;
      --muted: #666b76;
      --text: #17181c;
      --border: #d8dbe2;
      --accent: #2563eb;
      --selected: #eaf1ff;
    }
    @media (prefers-color-scheme: dark) {
      :root { --bg: #111216; --panel: #1b1d24; --muted: #9ca3af; --text: #f6f7fb; --border: #303541; --accent: #60a5fa; --selected: #172642; }
    }
    body { margin: 0; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, sans-serif; background: var(--bg); color: var(--text); }
    header { display: flex; justify-content: space-between; align-items: center; padding: 12px 24px; border-bottom: 1px solid var(--border); background: var(--panel); }
    header h1 { margin: 0; font-size: 14px; font-weight: 650; color: var(--muted); }
    main { padding: 28px; max-width: 1120px; margin: 0 auto; }
    h2 { margin: 0 0 8px; font-size: 28px; line-height: 1.15; }
    h3 { margin: 0 0 6px; font-size: 18px; }
    p { line-height: 1.55; }
    .subtitle { margin: 0 0 24px; color: var(--muted); }
    .options { display: grid; gap: 12px; }
    .option, .card { background: var(--panel); border: 2px solid var(--border); border-radius: 16px; padding: 16px; cursor: pointer; transition: border-color 120ms ease, background 120ms ease, transform 120ms ease; }
    .option:hover, .card:hover { border-color: var(--accent); transform: translateY(-1px); }
    .option.selected, .card.selected { border-color: var(--accent); background: var(--selected); }
    .option { display: flex; gap: 14px; align-items: flex-start; }
    .letter { width: 30px; height: 30px; flex: 0 0 30px; border-radius: 9px; display: grid; place-items: center; background: var(--bg); color: var(--muted); font-weight: 700; }
    .selected .letter { background: var(--accent); color: white; }
    .content p, .card p { margin: 0; color: var(--muted); }
    .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; }
    .mockup { background: var(--panel); border: 1px solid var(--border); border-radius: 16px; overflow: hidden; margin: 16px 0; }
    .mockup-header { padding: 10px 14px; border-bottom: 1px solid var(--border); color: var(--muted); font-size: 13px; }
    .mockup-body { padding: 18px; }
    .split { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px; }
    @media (max-width: 760px) { main { padding: 18px; } .split { grid-template-columns: 1fr; } }
    footer { position: sticky; bottom: 0; padding: 10px 24px; text-align: center; color: var(--muted); border-top: 1px solid var(--border); background: var(--panel); }
  </style>
</head>
<body>
  <header>
    <h1>Hotpot Visual Companion</h1>
    <span>Connected</span>
  </header>
  <main><!-- CONTENT --></main>
  <footer id="indicator-text">Click an option above, then return to the terminal.</footer>
</body>
</html>"#;

const HELPER_SCRIPT: &str = r#"(function () {
  function sendEvent(event) {
    const body = JSON.stringify(event);
    if (navigator.sendBeacon) {
      navigator.sendBeacon('/event', new Blob([body], { type: 'application/json' }));
      return;
    }
    fetch('/event', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body }).catch(() => {});
  }

  function selectedLabel(target) {
    const heading = target.querySelector('h3');
    return heading ? heading.textContent.trim() : target.dataset.choice;
  }

  window.toggleSelect = function (target) {
    const container = target.closest('.options, .cards');
    const multi = container && container.dataset.multiselect !== undefined;
    if (container && !multi) {
      container.querySelectorAll('.option, .card').forEach((item) => item.classList.remove('selected'));
    }
    target.classList.toggle('selected', multi ? !target.classList.contains('selected') : true);

    const indicator = document.getElementById('indicator-text');
    if (indicator) indicator.textContent = `${selectedLabel(target)} selected. Return to the terminal to continue.`;
  };

  document.addEventListener('click', (event) => {
    const target = event.target.closest('[data-choice]');
    if (!target) return;

    sendEvent({
      type: 'click',
      choice: target.dataset.choice,
      text: target.textContent.trim(),
      id: target.id || null,
    });
  });
})();"#;

pub struct StartOptions {
    pub project_dir: PathBuf,
    pub host: String,
    pub url_host: String,
    pub port: u16,
    pub daemon: bool,
}

pub struct ServeOptions {
    pub session_dir: PathBuf,
    pub host: String,
    pub url_host: String,
    pub port: u16,
    pub print_info: bool,
}

pub struct StopOptions {
    pub session_dir: Option<PathBuf>,
    pub all: bool,
}

#[derive(Serialize)]
struct ServerInfo {
    #[serde(rename = "type")]
    type_name: &'static str,
    pid: u32,
    port: u16,
    host: String,
    url_host: String,
    url: String,
    screen_dir: PathBuf,
    state_dir: PathBuf,
    session_dir: PathBuf,
}

pub fn start(options: StartOptions) -> Result<()> {
    let session_dir = make_session_dir(&options.project_dir);
    prepare_session_dirs(&session_dir)?;
    let port = if options.port == 0 {
        available_port(&options.host)?
    } else {
        options.port
    };

    if options.daemon {
        start_daemon(&session_dir, &options.host, &options.url_host, port)?;
        let info = wait_for_server_info(&session_dir)?;
        print!("{info}");
        return Ok(());
    }

    serve(ServeOptions {
        session_dir,
        host: options.host,
        url_host: options.url_host,
        port,
        print_info: true,
    })
}

pub fn serve(options: ServeOptions) -> Result<()> {
    prepare_session_dirs(&options.session_dir)?;
    let address = format!("{}:{}", options.host, options.port);
    let server = Server::http(&address).map_err(|err| anyhow!(err.to_string()))?;
    let info = write_server_info(&options)?;

    if options.print_info {
        println!("{}", serde_json::to_string(&info)?);
    }

    for request in server.incoming_requests() {
        if let Err(error) = handle_request(request, &options.session_dir) {
            eprintln!("hotpot server request error: {error:#}");
        }
    }

    Ok(())
}

pub fn stop(options: StopOptions) -> Result<()> {
    if options.all {
        let root_dir = context::resolve_root_dir(None)?;
        return stop_brainstorm_sessions(&root_dir);
    }

    let session_dir = options
        .session_dir
        .ok_or_else(|| anyhow!("--session-dir is required unless --all is used."))?;
    ensure_brainstorm_session_dir(&session_dir)?;
    stop_session(&session_dir)
}

/// Ensures a session directory stays under the project's brainstorm tree.
///
/// 确保 session 目录始终位于项目的 brainstorm 树下。
fn ensure_brainstorm_session_dir(session_dir: &Path) -> Result<()> {
    if !session_dir.exists() {
        return Ok(());
    }

    let session_dir = session_dir
        .canonicalize()
        .with_context(|| format!("确认 session 目录失败：{}", session_dir.display()))?;

    let allowed = session_dir.ancestors().any(|ancestor| {
        ancestor.file_name() == Some(OsStr::new("brainstorm"))
            && ancestor.parent().and_then(|parent| parent.file_name()) == Some(OsStr::new(".hotpot"))
    });

    if !allowed {
        bail!(
            "--session-dir must point to a .hotpot/brainstorm session directory: {}",
            session_dir.display()
        );
    }

    Ok(())
}

/// Stops every brainstorm session under the project root.
///
/// 停止项目根下的所有 brainstorm session。
fn stop_brainstorm_sessions(root_dir: &str) -> Result<()> {
    let brainstorm_dir = paths::hotpot_dir(root_dir).join("brainstorm");
    if !brainstorm_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&brainstorm_dir)
        .with_context(|| format!("读取目录失败：{}", brainstorm_dir.display()))?
    {
        let session_dir = entry?.path();
        if session_dir.is_dir() {
            stop_session(&session_dir)?;
        }
    }

    Ok(())
}

fn make_session_dir(project_dir: &Path) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    paths::hotpot_dir(&project_dir.to_string_lossy())
        .join("brainstorm")
        .join(format!("{millis}-{}", nanoid!(6)))
}

fn prepare_session_dirs(session_dir: &Path) -> Result<()> {
    fs::create_dir_all(content_dir(session_dir))
        .with_context(|| format!("创建内容目录失败：{}", content_dir(session_dir).display()))?;
    fs::create_dir_all(state_dir(session_dir))
        .with_context(|| format!("创建状态目录失败：{}", state_dir(session_dir).display()))?;
    Ok(())
}

fn start_daemon(session_dir: &Path, host: &str, url_host: &str, port: u16) -> Result<()> {
    let exe = std::env::current_exe().context("获取 hotpot 当前可执行文件路径失败")?;
    Command::new(exe)
        .arg("server")
        .arg("serve")
        .arg("--session-dir")
        .arg(session_dir)
        .arg("--host")
        .arg(host)
        .arg("--url-host")
        .arg(url_host)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("启动 hotpot server 后台进程失败")?;
    Ok(())
}

fn wait_for_server_info(session_dir: &Path) -> Result<String> {
    let path = state_dir(session_dir).join("server-info");
    for _ in 0..100 {
        if path.exists() {
            return fs::read_to_string(&path)
                .with_context(|| format!("读取 server-info 失败：{}", path.display()));
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!("server did not start within 5 seconds")
}

fn write_server_info(options: &ServeOptions) -> Result<ServerInfo> {
    let state_dir = state_dir(&options.session_dir);
    let info = ServerInfo {
        type_name: "server-started",
        pid: std::process::id(),
        port: options.port,
        host: options.host.clone(),
        url_host: options.url_host.clone(),
        url: format!("http://{}:{}", options.url_host, options.port),
        screen_dir: content_dir(&options.session_dir),
        state_dir: state_dir.clone(),
        session_dir: options.session_dir.clone(),
    };
    fs::write(state_dir.join("server.pid"), info.pid.to_string())
        .with_context(|| format!("写入 server.pid 失败：{}", state_dir.display()))?;
    fs::write(
        state_dir.join("server-info"),
        format!("{}\n", serde_json::to_string(&info)?),
    )
    .with_context(|| format!("写入 server-info 失败：{}", state_dir.display()))?;
    Ok(info)
}

fn handle_request(mut request: Request, session_dir: &Path) -> Result<()> {
    match (request.method(), request.url()) {
        (&Method::Get, "/") => respond_html(request, render_screen(session_dir)?),
        (&Method::Post, "/event") => {
            let mut body = String::new();
            request
                .as_reader()
                .read_to_string(&mut body)
                .context("读取事件请求失败")?;
            write_event(session_dir, &body)?;
            request
                .respond(Response::empty(StatusCode(204)))
                .map_err(|err| anyhow!(err.to_string()))?;
            Ok(())
        }
        _ => {
            request
                .respond(Response::from_string("Not found").with_status_code(StatusCode(404)))
                .map_err(|err| anyhow!(err.to_string()))?;
            Ok(())
        }
    }
}

fn respond_html(request: Request, body: String) -> Result<()> {
    let mut response = Response::from_string(body);
    response.add_header(
        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..])
            .map_err(|_| anyhow!("构造 Content-Type 响应头失败"))?,
    );
    request
        .respond(response)
        .map_err(|err| anyhow!(err.to_string()))?;
    Ok(())
}

fn render_screen(session_dir: &Path) -> Result<String> {
    let content = match latest_html_file(&content_dir(session_dir))? {
        Some(path) => fs::read_to_string(&path)
            .with_context(|| format!("读取 HTML 文件失败：{}", path.display()))?,
        None => FRAME_TEMPLATE.replace(
            "<!-- CONTENT -->",
            "<h2>Hotpot Visual Companion</h2><p class=\"subtitle\">Waiting for the agent to push a screen...</p>",
        ),
    };

    let trimmed = content.trim_start().to_lowercase();
    let html = if trimmed.starts_with("<!doctype") || trimmed.starts_with("<html") {
        content
    } else {
        FRAME_TEMPLATE.replace("<!-- CONTENT -->", &content)
    };

    Ok(inject_helper(&html))
}

fn inject_helper(html: &str) -> String {
    let script = format!("<script>{HELPER_SCRIPT}</script>");
    if let Some(index) = html.rfind("</body>") {
        let mut output = String::with_capacity(html.len() + script.len());
        output.push_str(&html[..index]);
        output.push_str(&script);
        output.push_str(&html[index..]);
        output
    } else {
        format!("{html}{script}")
    }
}

fn latest_html_file(dir: &Path) -> Result<Option<PathBuf>> {
    let mut latest: Option<(PathBuf, SystemTime)> = None;
    if !dir.exists() {
        return Ok(None);
    }

    for entry in fs::read_dir(dir).with_context(|| format!("读取目录失败：{}", dir.display()))?
    {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if latest.as_ref().is_none_or(|(_, time)| modified > *time) {
            latest = Some((path, modified));
        }
    }

    Ok(latest.map(|(path, _)| path))
}

fn write_event(session_dir: &Path, body: &str) -> Result<()> {
    let event = match serde_json::from_str::<serde_json::Value>(body.trim()) {
        Ok(mut value) => {
            if let serde_json::Value::Object(ref mut object) = value {
                object.insert(
                    "timestamp".to_string(),
                    serde_json::json!(timestamp_millis()),
                );
            }
            value
        }
        Err(error) => serde_json::json!({
            "type": "parse-error",
            "message": error.to_string(),
            "timestamp": timestamp_millis(),
        }),
    };

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state_dir(session_dir).join("events"))
        .with_context(|| format!("打开事件文件失败：{}", state_dir(session_dir).display()))?;
    writeln!(file, "{}", serde_json::to_string(&event)?)
        .with_context(|| format!("写入事件文件失败：{}", state_dir(session_dir).display()))?;
    Ok(())
}

fn stop_session(session_dir: &Path) -> Result<()> {
    stop_session_with(session_dir, session_is_alive, terminate_pid)
}

/// Stops one session and removes the session dir when cleanup is safe.
///
/// 停止单个 session，并在安全时清理整个 session 目录。
fn stop_session_with<FAlive, FTerminate>(
    session_dir: &Path,
    is_pid_alive: FAlive,
    terminate_pid_fn: FTerminate,
) -> Result<()>
where
    FAlive: Fn(u32) -> bool,
    FTerminate: Fn(u32) -> Result<()>,
{
    let pid_path = state_dir(session_dir).join("server.pid");
    let pid = match fs::read_to_string(&pid_path) {
        Ok(raw) => match raw.trim().parse::<u32>() {
            Ok(pid) => pid,
            Err(_) => return cleanup_session_dir(session_dir),
        },
        Err(_) => return cleanup_session_dir(session_dir),
    };

    if !is_pid_alive(pid) {
        return cleanup_session_dir(session_dir);
    }

    if let Err(err) = terminate_pid_fn(pid) {
        if !is_pid_alive(pid) {
            return cleanup_session_dir(session_dir);
        }
        return Err(err).context(format!("停止 server 进程失败，pid：{pid}"));
    }

    cleanup_session_dir(session_dir)
}

#[cfg(test)]
fn stop_brainstorm_sessions_for_test<F>(root_dir: &str, stop_session_fn: F) -> Result<()>
where
    F: Fn(&Path) -> Result<()>,
{
    let brainstorm_dir = paths::hotpot_dir(root_dir).join("brainstorm");
    if !brainstorm_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(&brainstorm_dir)
        .with_context(|| format!("读取目录失败：{}", brainstorm_dir.display()))?
    {
        let session_dir = entry?.path();
        if session_dir.is_dir() {
            stop_session_fn(&session_dir)?;
        }
    }

    Ok(())
}

/// Removes a session directory tree after stop or orphan cleanup.
///
/// 在停止或孤儿清理后移除整个 session 目录树。
fn cleanup_session_dir(session_dir: &Path) -> Result<()> {
    if !session_dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(session_dir)
        .with_context(|| format!("删除 session 目录失败：{}", session_dir.display()))
}

/// Checks whether a pid is alive on the current platform.
///
/// 检查 pid 在当前平台上是否仍然存活。
fn session_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
            }
            _ => false,
        }
    }
}

/// Terminates a live pid using the platform's native command.
///
/// 使用平台原生命令终止存活 pid。
fn terminate_pid(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        let status = Command::new("kill")
            .arg(pid.to_string())
            .status()
            .with_context(|| format!("停止 server 进程失败，pid：{pid}"))?;
        if !status.success() {
            bail!("停止 server 进程失败，pid：{pid}");
        }
    }
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .status()
            .with_context(|| format!("停止 server 进程失败，pid：{pid}"))?;
        if !status.success() {
            bail!("停止 server 进程失败，pid：{pid}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::Builder;

    fn temp_session_dir(label: &str) -> tempfile::TempDir {
        let dir = Builder::new()
            .prefix(&format!("hotpot-server-{label}-"))
            .tempdir()
            .unwrap();
        fs::create_dir_all(dir.path()).unwrap();
        dir
    }

    fn write_pid(session_dir: &Path, pid: &str) {
        fs::create_dir_all(state_dir(session_dir)).unwrap();
        fs::write(state_dir(session_dir).join("server.pid"), pid).unwrap();
    }

    #[test]
    fn stop_session_removes_live_session_dir_after_success() {
        let dir = temp_session_dir("live");
        let session_dir = dir.path();
        write_pid(session_dir, "1234");

        let alive = |_: u32| true;
        let terminate = |_: u32| Ok(());
        stop_session_with(session_dir, alive, terminate).unwrap();

        assert!(!session_dir.exists());
    }

    #[test]
    fn stop_session_cleans_orphan_without_pid() {
        let dir = temp_session_dir("orphan-missing-pid");
        let session_dir = dir.path();
        fs::create_dir_all(content_dir(session_dir)).unwrap();
        fs::create_dir_all(state_dir(session_dir).join("nested")).unwrap();
        fs::write(content_dir(session_dir).join("screen.html"), "<html/>").unwrap();

        stop_session_with(session_dir, |_| false, |_| Ok(())).unwrap();

        assert!(!session_dir.exists());
    }

    #[test]
    fn stop_session_is_idempotent_for_repeated_orphan_cleanup() {
        let dir = temp_session_dir("idempotent");
        let session_dir = dir.path();
        write_pid(session_dir, "9999");

        let alive = |_: u32| false;
        stop_session_with(session_dir, alive, |_| Ok(())).unwrap();
        stop_session_with(session_dir, alive, |_| Ok(())).unwrap();

        assert!(!session_dir.exists());
    }

    #[test]
    fn stop_brainstorm_sessions_reuses_single_cleanup_path() {
        let dir = temp_session_dir("all");
        let root = dir.path();
        let brainstorm = paths::hotpot_dir(&root.to_string_lossy()).join("brainstorm");
        let session_a = brainstorm.join("a");
        let session_b = brainstorm.join("b");
        write_pid(&session_a, "1");
        write_pid(&session_b, "2");

        let calls = std::cell::RefCell::new(Vec::new());
        stop_brainstorm_sessions_for_test(&root.to_string_lossy(), |session_dir| {
            calls.borrow_mut().push(session_dir.display().to_string());
            cleanup_session_dir(session_dir)
        })
        .unwrap();

        // The helper should have removed both sessions via the shared cleanup path.
        assert!(!session_a.exists());
        assert!(!session_b.exists());
        assert_eq!(calls.borrow().len(), 2);
    }

    #[test]
    /// Verifies stop refuses to clean directories outside brainstorm.
    ///
    /// 验证 stop 会拒绝清理 brainstorm 目录之外的路径。
    fn ensure_brainstorm_session_dir_rejects_outside_paths() {
        let dir = temp_session_dir("guard");
        let root = dir.path();
        let outside = root.join("outside");
        fs::create_dir_all(&outside).unwrap();

        let err = ensure_brainstorm_session_dir(&outside).expect_err("should reject outside path");
        let msg = format!("{err}");
        assert!(msg.contains("brainstorm"), "unexpected error: {msg}");
        assert!(outside.exists(), "guard must not delete arbitrary paths");
    }
}

fn available_port(host: &str) -> Result<u16> {
    let listener = TcpListener::bind(format!("{host}:0"))
        .with_context(|| format!("绑定临时端口失败，host：{host}"))?;
    Ok(listener.local_addr()?.port())
}

fn content_dir(session_dir: &Path) -> PathBuf {
    session_dir.join("content")
}

fn state_dir(session_dir: &Path) -> PathBuf {
    session_dir.join("state")
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
